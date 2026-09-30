#![forbid(unsafe_code)]
//! Binary `aurion-node`.
//!
//! Seluruh orkestrasi berada pada pustaka `aurion_node::node`; berkas ini
//! hanya menangani CLI, pemasangan logging, dan siklus hidup daemon.

use std::path::PathBuf;

use clap::Parser;

use aurion_config::{init_central_logging_with_settings, AurionSettings};
use aurion_criptografi::Keypair;

use aurion_node::node::{ChainEvent, ChainNode, NodeConfig, NodeHandle, DEFAULT_BLOCK_TIME_MS};

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file_config = AurionSettings::load_from_file("config/node.dev.toml")?;
    let cli = Cli::parse();
    let settings = apply_cli_overrides(file_config, &cli);

    let _log_guard =
        init_central_logging_with_settings(settings.mode, &settings.logging, "aurion-node");

    let proposer = Keypair::generate().public_key_bytes();
    let config = NodeConfig {
        chain_id: settings.network.chain_id,
        data_dir: settings.storage.db_path.clone(),
        treasury_account: [0xA1; 32],
        initial_validators: vec![proposer],
        initial_guards: aurion_node::node::default_guards(),
        block_proposer: proposer,
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

    // Task pengamat: meneruskan kejadian actor ke log terpusat.
    let observer = tokio::spawn({
        let token = token.clone();
        let mut events = runtime.events;
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

    let _ = join.await;
    let _ = observer.await;
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
