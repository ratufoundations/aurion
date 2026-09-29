#![forbid(unsafe_code)]

use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::OwnedSemaphorePermit;

use clap::Parser;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, timeout};

use aurion_config::{init_central_logging_with_settings, AurionSettings};
use aurion_consensus::{RoundState, ValidatorSet, Vote, VoteType};
use aurion_core::ModuleDispatcher;
use aurion_core::{Account, Block, BlockHeader, State, Transaction};
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};
use aurion_ledger::LedgerStore;
use aurion_mempool::{Mempool, MempoolConfig};
use aurion_network::{Handshake, NetworkMessage, PeerConnection};

#[derive(Parser, Debug)]
#[command(
    name = "aurion-node",
    version = "0.1.0",
    about = "Simpul Validator Aurion Blockchain"
)]
struct Cli {
    #[arg(short, long, help = "Port TCP untuk P2P networking")]
    port: Option<u16>,
    #[arg(short, long, help = "Jalur direktori database ledger (redb)")]
    data_dir: Option<PathBuf>,
    #[arg(short, long, help = "Chain ID jaringan")]
    chain_id: Option<u64>,
    #[arg(
        long,
        help = "Alamat peer awal untuk koneksi bootstrap (contoh: 127.0.0.1:9001)"
    )]
    peer: Option<String>,
    #[arg(
        long,
        default_value_t = true,
        help = "Jalankan demo generator transaksi internal"
    )]
    demo: bool,
}

struct NodeContext {
    pub validator_keypair: Keypair,
    pub chain_id: u64,
    pub handshake_timeout_ms: u64,
    pub state: Mutex<State>,
    pub ledger: LedgerStore,
    pub mempool: Mutex<Mempool>,
    pub validator_set: ValidatorSet,
    pub module_dispatcher: ModuleDispatcher,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Muat dan validasi konfigurasi terpusat
    let file_config = AurionSettings::load_from_file("config/node.dev.toml")?;
    let cli = Cli::parse();
    // 2. Nilai CLI menimpa berkas bila flag terkait diberikan eksplisit
    let settings = apply_cli_overrides(file_config, &cli);

    // 3. Pasang logging terpusat sesuai mode
    let _guard =
        init_central_logging_with_settings(settings.mode, &settings.logging, "aurion-node");

    tracing::info!(
        chain_id = settings.network.chain_id,
        db_path = %settings.storage.db_path.display(),
        "Simpul Aurion berhasil diinisialisasi dari konfigurasi terpusat"
    );
    run_node(cli, settings).await
}

/// Titik pemasangan modul aplikasi. Modul baru cukup didaftarkan di fungsi ini.
fn build_module_dispatcher() -> ModuleDispatcher {
    let dispatcher = ModuleDispatcher::new();
    // Contoh pemasangan: dispatcher.register_module(Box::new(StakingModule::new()))?;
    dispatcher
}

/// Nilai CLI (`--port/--chain-id/--data-dir`) menimpa konfigurasi berkas
/// hanya bila flag tersebut diberikan eksplisit di baris perintah.
fn apply_cli_overrides(mut settings: AurionSettings, cli: &Cli) -> AurionSettings {
    use std::env::args;
    let raw: Vec<String> = args().collect();
    let has = |flag: &str| {
        raw.iter()
            .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
    };
    if has("--port") || has("-p") {
        if let Some(host) = settings
            .network
            .p2p_bind_addr
            .rsplit_once(':')
            .map(|(h, _)| h.to_string())
        {
            if let Some(port) = cli.port {
                settings.network.p2p_bind_addr = format!("{host}:{port}");
            }
        }
    }
    if has("--chain-id") || has("-c") {
        if let Some(chain_id) = cli.chain_id {
            settings.network.chain_id = chain_id;
        }
    }
    if has("--data-dir") || has("-d") {
        if let Some(data_dir) = &cli.data_dir {
            settings.storage.db_path.clone_from(data_dir);
        }
    }
    settings
}

// Fungsi ini mengoordinasikan lifecycle node, listener P2P, demo, dan block producer.
#[allow(clippy::too_many_lines)]
async fn run_node(
    cli: Cli,
    settings: AurionSettings,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("============================================================");
    tracing::info!("               AURION BLOCKCHAIN VALIDATOR NODE             ");
    tracing::info!("============================================================");
    let validator_keypair = Keypair::generate();
    let validator_pubkey = validator_keypair.public_key_bytes();
    tracing::info!(
        "Identitas Validator (Pubkey): {:02X?}",
        &validator_pubkey[0..8]
    );
    tracing::info!(
        "Chain ID                     : {}",
        settings.network.chain_id
    );
    tracing::info!(
        "P2P Bind Addr                : {}",
        settings.network.p2p_bind_addr
    );
    let db_path = settings.storage.db_path.clone();
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let ledger = LedgerStore::open(&db_path)?;
    tracing::info!("Ledger Store                 : Terbuka di {:?}", db_path);
    let mut state = State::new();
    let current_height = ledger.get_latest_height()?;
    tracing::info!("Ledger Block Height Saat Ini : {}", current_height);
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    if current_height == 0 {
        tracing::info!("\n[GENESIS] Menginisialisasi Alokasi Treasury Akun Genesis...");
        state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
        state.insert_account(validator_pubkey, Account::new(500_000, 0));
        tracing::info!("  - Akun Alice     : 1.000.000 Quanta");
        tracing::info!("  - Akun Validator : 500.000 Quanta");
    }
    let module_dispatcher = build_module_dispatcher();
    module_dispatcher.init_genesis(&mut state)?;
    let validator_set = ValidatorSet::new(vec![validator_pubkey]);
    let mempool = Mempool::new(MempoolConfig::default());
    let ctx = Arc::new(NodeContext {
        validator_keypair,
        chain_id: settings.network.chain_id,
        handshake_timeout_ms: settings.network.handshake_timeout_ms,
        state: Mutex::new(state),
        ledger,
        mempool: Mutex::new(mempool),
        validator_set,
        module_dispatcher,
    });
    tracing::info!(
        module_count = ctx.module_dispatcher.module_ids().count(),
        "Module runtime siap"
    );
    let listen_addr: SocketAddr = settings.network.p2p_bind_addr.parse()?;
    let listener = TcpListener::bind(listen_addr).await?;
    let peer_slots = Arc::new(Semaphore::new(usize::try_from(settings.network.max_peers)?));
    tracing::info!("\n[NETWORK] Listener P2P aktif pada: {}", listen_addr);
    let net_ctx = Arc::clone(&ctx);
    let peer_limit = Arc::clone(&peer_slots);
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, remote_addr)) => {
                    tracing::info!("[NETWORK] Peer baru terhubung dari: {}", remote_addr);
                    match peer_limit.clone().try_acquire_owned() {
                        Ok(permit) => {
                            let peer_ctx = Arc::clone(&net_ctx);
                            tokio::spawn(async move {
                                handle_peer_inbound(socket, peer_ctx, permit).await;
                            });
                        }
                        Err(_) => {
                            tracing::warn!(peer = %remote_addr, "Batas peer jaringan tercapai");
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[NETWORK ERROR] Gagal menerima koneksi TCP: {}", e);
                }
            }
        }
    });
    if let Some(peer_addr) = cli.peer {
        tracing::info!("[NETWORK] Menyambung ke bootstrap peer: {}", peer_addr);
        let connect_ctx = Arc::clone(&ctx);
        let local_port = listen_addr.port();
        tokio::spawn(async move {
            match TcpStream::connect(&peer_addr).await {
                Ok(stream) => {
                    let mut peer = PeerConnection::new(stream);
                    let hs = Handshake {
                        node_id: connect_ctx.validator_keypair.public_key_bytes(),
                        chain_id: connect_ctx.chain_id,
                        listen_port: local_port,
                    };
                    let _ = peer.send_message(NetworkMessage::Handshake(hs)).await;
                    tracing::info!("[NETWORK] Handshake berhasil dikirim ke {}", peer_addr);
                }
                Err(e) => tracing::error!(
                    "[NETWORK ERROR] Gagal menyambung ke peer {}: {}",
                    peer_addr,
                    e
                ),
            }
        });
    }
    if cli.demo {
        let demo_ctx = Arc::clone(&ctx);
        let alice_kp = alice;
        let bob_pubkey = bob.public_key_bytes();
        tokio::spawn(async move {
            let mut nonce = 0u64;
            loop {
                sleep(Duration::from_secs(3)).await;
                let sender: PublicKeyBytes = alice_kp.public_key_bytes();
                let amount = 25_000u64;
                let fee = 10u64;
                let payload = Transaction::payload_bytes(&sender, &bob_pubkey, amount, nonce, fee);
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"AURION_TX_CANONICAL_V1");
                hasher.update(&payload);
                let digest: Hash256 = *hasher.finalize().as_bytes();
                let sig = alice_kp.sign(&digest);
                let tx = Transaction::new(sender, bob_pubkey, amount, nonce, fee, sig);
                let state_guard = demo_ctx.state.lock().await;
                let mut mempool_guard = demo_ctx.mempool.lock().await;
                match mempool_guard.insert(tx.clone(), &state_guard) {
                    Ok(()) => {
                        tracing::info!("\n[MEMPOOL] Transaksi Demo Masuk -> Kirim {} Quanta ke Bob (Nonce: {})", amount, nonce);
                        nonce += 1;
                    }
                    Err(e) => {
                        tracing::error!("[MEMPOOL REJECT] Transaksi ditolak: {}", e);
                    }
                }
            }
        });
    }
    tracing::info!("\n[ENGINE] Mesin Konsensus BFT & Block Producer Berjalan...\n");
    let engine_ctx = Arc::clone(&ctx);
    loop {
        sleep(Duration::from_millis(settings.consensus.block_time_ms)).await;
        let mut mempool_guard = engine_ctx.mempool.lock().await;
        if mempool_guard.is_empty() {
            continue;
        }
        let mut state_guard = engine_ctx.state.lock().await;
        let max_tx_per_block = usize::try_from(settings.consensus.max_tx_per_block)?;
        let txs_to_mine =
            mempool_guard.select_transactions_for_block(&state_guard, max_tx_per_block);
        if txs_to_mine.is_empty() {
            continue;
        }
        let current_height = engine_ctx.ledger.get_latest_height().unwrap_or(0);
        let next_height = current_height + 1;
        let proposer = engine_ctx.validator_keypair.public_key_bytes();
        let mut shadow_state = state_guard.clone();
        let mut executed_txs = Vec::new();
        for tx in txs_to_mine {
            match shadow_state.apply_transaction_with_proposer(&tx, proposer) {
                Ok(()) => executed_txs.push(tx),
                Err(e) => tracing::error!("[EXECUTION SKIP] Tx dilewati karena error FSM: {}", e),
            }
        }
        if executed_txs.is_empty() {
            continue;
        }
        let state_root = shadow_state.compute_state_root();
        let prev_hash = match engine_ctx.ledger.get_block_by_height(current_height) {
            Ok(Some(prev_block)) => prev_block.header.hash(),
            _ => [0u8; 32],
        };
        let timestamp = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
        let candidate_block = Block {
            header: BlockHeader {
                height: next_height,
                prev_hash,
                state_root,
                tx_count: u32::try_from(executed_txs.len())?,
                timestamp,
                proposer,
            },
            transactions: executed_txs,
        };
        let block_hash = candidate_block.header.hash();
        let mut round_state = RoundState::new(next_height, 0, engine_ctx.validator_set.clone());
        let prevote = create_vote(
            &engine_ctx.validator_keypair,
            block_hash,
            next_height,
            0,
            VoteType::Prevote,
        );
        let prevote_qc = match round_state.add_vote(&prevote) {
            Ok(qc) => qc,
            Err(e) => {
                tracing::error!("[BFT SKIP] Prevote gagal: {}", e);
                continue;
            }
        };
        if prevote_qc.is_some() {
            let precommit = create_vote(
                &engine_ctx.validator_keypair,
                block_hash,
                next_height,
                0,
                VoteType::Precommit,
            );
            let precommit_qc = match round_state.add_vote(&precommit) {
                Ok(qc) => qc,
                Err(e) => {
                    tracing::error!("[BFT SKIP] Precommit gagal: {}", e);
                    continue;
                }
            };
            if let Some(qc) = precommit_qc {
                match engine_ctx
                    .ledger
                    .commit_block(&candidate_block, &mut state_guard)
                {
                    Ok(()) => {
                        mempool_guard.prune_committed(&candidate_block, &state_guard);
                        tracing::info!(
                            "------------------------------------------------------------"
                        );
                        tracing::info!(">> BLOK BERHASIL DI-COMMIT KE LEDGER (ACID PERSISTED) <<");
                        tracing::info!("   Tinggi Blok (Height) : {}", next_height);
                        tracing::info!("   Hash Blok            : {:02X?}", &block_hash[0..8]);
                        tracing::info!("   State Root           : {:02X?}", &state_root[0..8]);
                        tracing::info!(
                            "   Jumlah Transaksi     : {}",
                            candidate_block.header.tx_count
                        );
                        tracing::info!("   BFT Quorum Signers   : {} validator", qc.signers.len());
                        tracing::info!(
                            "------------------------------------------------------------"
                        );
                    }
                    Err(e) => tracing::error!("[LEDGER ERROR] Commit blok gagal: {}", e),
                }
            }
        }
    }
}

async fn handle_peer_inbound(
    socket: TcpStream,
    ctx: Arc<NodeContext>,
    _permit: OwnedSemaphorePermit,
) {
    let mut peer = PeerConnection::new(socket);
    let first_message = timeout(
        Duration::from_millis(ctx.handshake_timeout_ms),
        peer.read_message(),
    )
    .await;
    let handshake = match first_message {
        Ok(Ok(Some(NetworkMessage::Handshake(handshake)))) => handshake,
        Ok(Ok(Some(_))) => {
            tracing::warn!("Peer tidak mengirim handshake sebagai pesan pertama");
            return;
        }
        Ok(Ok(None)) => return,
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "Gagal membaca handshake peer");
            return;
        }
        Err(_) => {
            tracing::warn!(
                timeout_ms = ctx.handshake_timeout_ms,
                "Handshake peer melewati batas waktu"
            );
            return;
        }
    };
    if handshake.chain_id != ctx.chain_id {
        tracing::warn!(
            peer_chain_id = handshake.chain_id,
            local_chain_id = ctx.chain_id,
            "Handshake peer ditolak: chain ID tidak cocok"
        );
        return;
    }
    tracing::info!(peer = ?handshake.node_id, chain_id = handshake.chain_id, listen_port = handshake.listen_port, "Handshake peer P2P berhasil");

    while let Ok(Some(msg)) = peer.read_message().await {
        match msg {
            NetworkMessage::Handshake(_) => {
                tracing::warn!("Peer mengirim handshake berulang");
            }
            NetworkMessage::Transaction(tx) => {
                let state_guard = ctx.state.lock().await;
                let mut mempool_guard = ctx.mempool.lock().await;
                let _ = mempool_guard.insert(tx, &state_guard);
            }
            NetworkMessage::Ping(nonce) => {
                let _ = peer.send_message(NetworkMessage::Pong(nonce)).await;
            }
            _ => {}
        }
    }
}

fn create_vote(
    keypair: &Keypair,
    block_hash: Hash256,
    height: u64,
    round: u32,
    vote_type: VoteType,
) -> Vote {
    let validator = keypair.public_key_bytes();
    let temp_vote = Vote::new(validator, block_hash, height, round, vote_type, [0u8; 64]);
    let digest = temp_vote.digest();
    let signature = keypair.sign(&digest);
    Vote::new(validator, block_hash, height, round, vote_type, signature)
}
