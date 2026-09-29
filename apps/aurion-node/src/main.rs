#![forbid(unsafe_code)]

use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::sleep;

use aurion_config::{init_central_logging, AurionConfig};
use aurion_consensus::{RoundState, ValidatorSet, Vote, VoteType};
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
    #[arg(
        short,
        long,
        default_value = "9000",
        help = "Port TCP untuk P2P networking"
    )]
    port: u16,
    #[arg(
        short,
        long,
        default_value = "./data/aurion.db",
        help = "Jalur direktori database ledger (redb)"
    )]
    data_dir: PathBuf,
    #[arg(short, long, default_value = "1001", help = "Chain ID jaringan")]
    chain_id: u64,
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
    pub state: Mutex<State>,
    pub ledger: LedgerStore,
    pub mempool: Mutex<Mempool>,
    pub validator_set: ValidatorSet,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Muat konfigurasi terpusat (fallback ke default bila berkas belum ada)
    let file_config = AurionConfig::load_from_file("config/node.dev.toml").ok();
    let cli = Cli::parse();
    // 2. Nilai CLI menimpa berkas bila flag terkait diberikan eksplisit
    let config = apply_cli_overrides(file_config.unwrap_or_default(), &cli);

    // 3. Pasang logging terpusat sesuai mode
    let _guard = init_central_logging(config.mode, &config.log_dir, "aurion-node");

    tracing::info!(
        chain_id = config.network.chain_id,
        db_path = %config.db_path,
        "Simpul Aurion berhasil diinisialisasi dari konfigurasi terpusat"
    );
    run_node(cli, config).await
}

/// Nilai CLI (`--port/--chain-id/--data-dir`) menimpa konfigurasi berkas
/// hanya bila flag tersebut diberikan eksplisit di baris perintah.
fn apply_cli_overrides(mut config: AurionConfig, cli: &Cli) -> AurionConfig {
    use std::env::args;
    let raw: Vec<String> = args().collect();
    let has = |flag: &str| {
        raw.iter()
            .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
    };
    if has("--port") {
        let mut net = config.network.clone();
        if let Some(host) = net
            .p2p_bind_addr
            .rsplit_once(':')
            .map(|(h, _)| h.to_string())
        {
            net.p2p_bind_addr = format!("{host}:{}", cli.port);
            config.network = net;
        }
    }
    if has("--chain-id") {
        config.network.chain_id = cli.chain_id;
    }
    if has("--data-dir") || has("--data_dir") {
        config.db_path = cli.data_dir.to_string_lossy().into_owned();
    }
    config
}

async fn run_node(
    cli: Cli,
    config: AurionConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("============================================================");
    println!("               AURION BLOCKCHAIN VALIDATOR NODE             ");
    println!("============================================================");
    let validator_keypair = Keypair::generate();
    let validator_pubkey = validator_keypair.public_key_bytes();
    println!(
        "Identitas Validator (Pubkey): {:02X?}",
        &validator_pubkey[0..8]
    );
    println!("Chain ID                     : {}", config.network.chain_id);
    println!(
        "P2P Bind Addr                : {}",
        config.network.p2p_bind_addr
    );
    let db_path = std::path::PathBuf::from(&config.db_path);
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let ledger = LedgerStore::open(&db_path)?;
    println!("Ledger Store                 : Terbuka di {:?}", db_path);
    let mut state = State::new();
    let current_height = ledger.get_latest_height()?;
    println!("Ledger Block Height Saat Ini : {}", current_height);
    let alice = Keypair::generate();
    let bob = Keypair::generate();
    if current_height == 0 {
        println!("\n[GENESIS] Menginisialisasi Alokasi Treasury Akun Genesis...");
        state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
        state.insert_account(validator_pubkey, Account::new(500_000, 0));
        println!("  - Akun Alice     : 1.000.000 Quanta");
        println!("  - Akun Validator : 500.000 Quanta");
    }
    let validator_set = ValidatorSet::new(vec![validator_pubkey]);
    let mempool = Mempool::new(MempoolConfig::default());
    let ctx = Arc::new(NodeContext {
        validator_keypair,
        chain_id: config.network.chain_id,
        state: Mutex::new(state),
        ledger,
        mempool: Mutex::new(mempool),
        validator_set,
    });
    let listen_addr: SocketAddr = match config.network.p2p_bind_addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!(
                "[CONFIG ERROR] p2p_bind_addr tidak valid ({}): {}",
                config.network.p2p_bind_addr, e
            );
            SocketAddr::from(([0, 0, 0, 0], cli.port))
        }
    };
    let listener = TcpListener::bind(listen_addr).await?;
    println!("\n[NETWORK] Listener P2P aktif pada: {}", listen_addr);
    let net_ctx = Arc::clone(&ctx);
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, remote_addr)) => {
                    println!("[NETWORK] Peer baru terhubung dari: {}", remote_addr);
                    let peer_ctx = Arc::clone(&net_ctx);
                    tokio::spawn(async move {
                        handle_peer_inbound(socket, peer_ctx).await;
                    });
                }
                Err(e) => {
                    eprintln!("[NETWORK ERROR] Gagal menerima koneksi TCP: {}", e);
                }
            }
        }
    });
    if let Some(peer_addr) = cli.peer {
        println!("[NETWORK] Menyambung ke bootstrap peer: {}", peer_addr);
        let connect_ctx = Arc::clone(&ctx);
        let local_port = cli.port;
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
                    println!("[NETWORK] Handshake berhasil dikirim ke {}", peer_addr);
                }
                Err(e) => eprintln!(
                    "[NETWORK ERROR] Gagal menyambung ke peer {}: {}",
                    peer_addr, e
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
                let payload = Transaction::payload_bytes(&sender, &bob_pubkey, amount, nonce);
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"AURION_TX_CANONICAL_V1");
                hasher.update(&payload);
                let digest: Hash256 = *hasher.finalize().as_bytes();
                let sig = alice_kp.sign(&digest);
                let tx = Transaction::new(sender, bob_pubkey, amount, nonce, sig);
                let state_guard = demo_ctx.state.lock().await;
                let mut mempool_guard = demo_ctx.mempool.lock().await;
                match mempool_guard.insert(tx.clone(), fee, &state_guard) {
                    Ok(()) => {
                        println!("\n[MEMPOOL] Transaksi Demo Masuk -> Kirim {} Quanta ke Bob (Nonce: {})", amount, nonce);
                        nonce += 1;
                    }
                    Err(e) => {
                        eprintln!("[MEMPOOL REJECT] Transaksi ditolak: {}", e);
                    }
                }
            }
        });
    }
    println!("\n[ENGINE] Mesin Konsensus BFT & Block Producer Berjalan...\n");
    let engine_ctx = Arc::clone(&ctx);
    loop {
        sleep(Duration::from_secs(2)).await;
        let mut mempool_guard = engine_ctx.mempool.lock().await;
        if mempool_guard.is_empty() {
            continue;
        }
        let mut state_guard = engine_ctx.state.lock().await;
        let txs_to_mine = mempool_guard.select_transactions_for_block(&state_guard, 50);
        if txs_to_mine.is_empty() {
            continue;
        }
        let current_height = engine_ctx.ledger.get_latest_height().unwrap_or(0);
        let next_height = current_height + 1;
        let mut shadow_state = state_guard.clone();
        let mut executed_txs = Vec::new();
        for tx in txs_to_mine {
            match shadow_state.apply_transaction(&tx) {
                Ok(()) => executed_txs.push(tx),
                Err(e) => eprintln!("[EXECUTION SKIP] Tx dilewati karena error FSM: {}", e),
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
        let candidate_block = Block {
            header: BlockHeader {
                height: next_height,
                prev_hash,
                state_root,
                tx_count: executed_txs.len() as u32,
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
                eprintln!("[BFT SKIP] Prevote gagal: {}", e);
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
                    eprintln!("[BFT SKIP] Precommit gagal: {}", e);
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
                        println!("------------------------------------------------------------");
                        println!(">> BLOK BERHASIL DI-COMMIT KE LEDGER (ACID PERSISTED) <<");
                        println!("   Tinggi Blok (Height) : {}", next_height);
                        println!("   Hash Blok            : {:02X?}", &block_hash[0..8]);
                        println!("   State Root           : {:02X?}", &state_root[0..8]);
                        println!(
                            "   Jumlah Transaksi     : {}",
                            candidate_block.header.tx_count
                        );
                        println!("   BFT Quorum Signers   : {} validator", qc.signers.len());
                        println!("------------------------------------------------------------");
                    }
                    Err(e) => eprintln!("[LEDGER ERROR] Commit blok gagal: {}", e),
                }
            }
        }
    }
}

async fn handle_peer_inbound(socket: TcpStream, ctx: Arc<NodeContext>) {
    let mut peer = PeerConnection::new(socket);
    while let Ok(Some(msg)) = peer.read_message().await {
        match msg {
            NetworkMessage::Handshake(hs) => {
                println!(
                    "[NETWORK] Peer Handshake: Node {:02X?} (Chain: {}, Port: {})",
                    &hs.node_id[0..6],
                    hs.chain_id,
                    hs.listen_port
                );
            }
            NetworkMessage::Transaction(tx) => {
                let state_guard = ctx.state.lock().await;
                let mut mempool_guard = ctx.mempool.lock().await;
                let _ = mempool_guard.insert(tx, 0, &state_guard);
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
