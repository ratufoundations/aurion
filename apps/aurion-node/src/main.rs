#![forbid(unsafe_code)]
#![allow(
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
//! Binary `aurion-node`.
//!
//! Seluruh orkestrasi berada pada pustaka `aurion_node::node`; berkas ini
//! hanya menangani CLI, pemasangan logging, driver jaringan P2P, dan siklus
//! hidup daemon.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use aurion_config::{init_central_logging_with_settings, AurionSettings};
use aurion_consensus::VoteType;
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};
use aurion_network::NetworkMessage;

use aurion_node::node::{
    default_guards, sign_vote, ChainEvent, ChainNode, NodeConfig, NodeHandle, DEFAULT_BLOCK_TIME_MS,
};
use aurion_node::p2p::{P2PConfig, P2PRuntime};

#[derive(Parser, Debug)]
#[command(
    name = "aurion-node",
    version = "0.1.0",
    about = "Simpul Validator Aurion Blockchain"
)]
struct Cli {
    #[arg(short, long, help = "Port TCP untuk jaringan P2P")]
    port: Option<u16>,
    #[arg(short, long, help = "Jalur direktori database ledger (redb)")]
    data_dir: Option<PathBuf>,
    #[arg(short, long, help = "Chain ID jaringan")]
    chain_id: Option<u64>,
    #[arg(
        long,
        value_name = "INDEX",
        default_value_t = 0,
        help = "Indeks identitas deterministik simpul (roster dev)"
    )]
    identity: usize,
    #[arg(
        long,
        value_name = "N",
        default_value_t = 1,
        help = "Ukuran roster validator klaster dev"
    )]
    roster_size: usize,
    #[arg(long = "peer", value_name = "ADDR", action = clap::ArgAction::Append, help = "Alamat seed peer yang didial")]
    peers: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file_config = AurionSettings::load_from_file("config/node.dev.toml")?;
    let cli = Cli::parse();
    let mut settings = apply_cli_overrides(file_config, &cli);
    if cli.data_dir.is_none() {
        settings.storage.db_path = PathBuf::from(format!("target/dev-db-node-{}", cli.identity));
    }

    let _log_guard =
        init_central_logging_with_settings(settings.mode, &settings.logging, "aurion-node");

    // Roster dev deterministik: kunci diturunkan dari blake3(index), sehingga
    // seluruh proses klaster berbagi himpunan validator yang sama persis.
    let roster_size = cli.roster_size.max(1);
    let identity = cli.identity % roster_size;
    let roster_keys = dev_roster(roster_size);
    let roster: Vec<PublicKeyBytes> = roster_keys.iter().map(Keypair::public_key_bytes).collect();
    let proposer = dev_key(identity);

    let config = NodeConfig {
        chain_id: settings.network.chain_id,
        data_dir: settings.storage.db_path.clone(),
        treasury_account: [0xA1; 32],
        initial_validators: roster.clone(),
        initial_guards: default_guards(),
        block_proposer: proposer.public_key_bytes(),
        max_tx_per_block: settings.consensus.max_tx_per_block,
        bootstrap: true,
        ..NodeConfig::default()
    };

    let node = ChainNode::bootstrap(config)?;
    tracing::info!(
        chain_id = settings.network.chain_id,
        height = node.last_height(),
        "Aurion node siap"
    );

    let mut runtime = NodeHandle::spawn(node);
    let handle = runtime.handle.clone();
    let token = runtime.token.clone();
    let join = &mut runtime.join;

    let p2p_config = P2PConfig {
        node_id: proposer.public_key_bytes(),
        chain_id: settings.network.chain_id,
        listen_addr: settings.network.p2p_bind_addr.clone(),
        max_peers: settings.network.max_peers as usize,
        seeds: cli.peers.clone(),
    };
    let p2p = P2PRuntime::spawn(p2p_config, handle.clone()).await?;
    let p2p_handle = p2p.handle.clone();

    println!("AURION_LISTEN:{}", p2p_handle.local_addr());
    let _ = std::io::stdout().flush();

    // Peserta BFT: tiap interval, geser ronde produksi blok lalu tandatangani
    // suara precommit untuk kandidat deterministik pada tinggi berjalan.
    let quorum_target = quorum_target(roster_size);
    let daemon = tokio::spawn({
        let handle = handle.clone();
        let p2p = p2p_handle.clone();
        let roster = roster.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_millis(DEFAULT_BLOCK_TIME_MS));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await;
            let cancel = handle.cancellation_token();
            loop {
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => break,
                    _ = interval.tick() => {
                        let _ = handle.produce_block();
                        if let Ok(latest) = handle.latest_height().await {
                            let height = latest + 1;
                            let vote = sign_vote(
                                &dev_key(identity),
                                candidate_hash(&roster, height),
                                height,
                                0,
                                VoteType::Precommit,
                            );
                            p2p.broadcast(NetworkMessage::Vote(vote));
                        }
                    }
                }
            }
        }
    });

    // Task pengamat: memonitor konsensus (block, votes) hingga kuorum BFT dan
    // meneruskan kejadian actor ke log terpusat.
    let observer = tokio::spawn({
        let token = token.clone();
        let mut events = runtime.events;
        let mut announced = false;
        async move {
            loop {
                tokio::select! {
                    biased;
                    () = token.cancelled() => break,
                    event = events.recv() => match event {
                        Some(ChainEvent::BlockCommitted { height, tx_count, .. }) => {
                            tracing::info!(height, tx_count, "Blok dikomit");
                        }
                        Some(ChainEvent::GenesisCommitted { height, .. }) => {
                            tracing::info!(height, "Genesis dikomit");
                        }
                        Some(ChainEvent::VotesObserved { height, votes }) => {
                            tracing::info!(height, votes, "Suara BFT teramati");
                            if votes >= quorum_target && !announced {
                                announced = true;
                                println!("AURION_QUORUM_READY");
                                let _ = std::io::stdout().flush();
                            }
                        }
                        Some(ChainEvent::ShutdownComplete { last_height }) => {
                            tracing::info!(last_height, "Node berhenti pada tinggi terakhir");
                            break;
                        }
                        Some(_) => {}
                        None => break,
                    }
                }
            }
        }
    });

    tracing::info!(
        interval_ms = DEFAULT_BLOCK_TIME_MS,
        "Tekan Ctrl+C untuk berhenti anggun"
    );
    tokio::signal::ctrl_c().await?;
    tracing::info!("Sinyal interupsi diterima, memulai graceful shutdown");
    handle.shutdown();
    p2p_handle.shutdown();

    let _ = daemon.await;
    let _ = join.await;
    let _ = observer.await;
    let _ = p2p.join.await;
    Ok(())
}

/// Nilai CLI menimpa konfigurasi berkas hanya bila flag diberikan eksplisit.
fn apply_cli_overrides(mut settings: AurionSettings, cli: &Cli) -> AurionSettings {
    let has_port = cli.port.is_some();
    let has_chain_id = cli.chain_id.is_some();

    if has_port {
        if let Some((host, _)) = settings.network.p2p_bind_addr.rsplit_once(':') {
            let host = host.to_string();
            if let Some(port) = cli.port {
                settings.network.p2p_bind_addr = format!("{host}:{port}");
            }
        }
    }
    if let Some(chain_id) = cli.chain_id {
        if has_chain_id {
            settings.network.chain_id = chain_id;
        }
    }
    if let Some(data_dir) = &cli.data_dir {
        settings.storage.db_path.clone_from(data_dir);
    }
    settings
}

/// Kunci dev deterministik: turunan blake3 dari indeks (bukan berangkat acak).
#[must_use]
fn dev_key(index: usize) -> Keypair {
    let digest = blake3::hash(&index.to_le_bytes());
    let mut seed = [0u8; 32];
    seed.copy_from_slice(digest.as_bytes());
    Keypair::from_bytes(&seed)
}

/// Kunci dev untuk seluruh indeks `0..size`, deterministik dan berbagi.
#[must_use]
fn dev_roster(size: usize) -> Vec<Keypair> {
    (0..size).map(dev_key).collect()
}

/// Kandidat blok deterministik untuk satu tinggi: hash roster || tinggi.
#[must_use]
fn candidate_hash(roster: &[PublicKeyBytes], height: u64) -> Hash256 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"AURION_CANDIDATE_BFT_V1");
    for key in roster {
        hasher.update(key);
    }
    hasher.update(&height.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// Ambang kuorum BFT `ceil(2n/3)` dihitung dengan bilangan bulat murni.
#[must_use]
fn quorum_target(size: usize) -> usize {
    let n = size.max(1);
    n - n / 3
}
