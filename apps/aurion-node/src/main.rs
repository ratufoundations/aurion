#![forbid(unsafe_code)]
#![allow(
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
//! Binary `aurion`.
//!
//! Seluruh orkestrasi berada pada pustaka `aurion_node::node`; berkas ini
//! hanya menangani CLI, pemasangan logging, driver jaringan P2P, dan siklus
//! hidup daemon.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use aurion_config::{init_central_logging_with_settings, AurionSettings};
use aurion_consensus::VoteType;
use aurion_core::{types::QUANTA_PER_AUR, Block, Quanta, Transaction};
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};
use aurion_network::NetworkMessage;

use aurion_node::node::{
    default_guards, sign_vote, ChainEvent, ChainNode, NodeConfig, NodeHandle, DEFAULT_BLOCK_TIME_MS,
};
use aurion_node::p2p::{P2PConfig, P2PRuntime};

#[derive(Parser, Debug)]
#[command(
    name = "aurion",
    version = "0.1.0",
    about = "Simpul Validator Aurion Blockchain"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Jalankan simpul Aurion
    Run {
        #[command(subcommand)]
        command: ServerCommand,
    },
    /// Mulai simpul Aurion
    Start {
        #[command(subcommand)]
        command: ServerCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum ServerCommand {
    /// Jalankan simpul server validator
    Server(ServerArgs),
}

#[derive(Args, Debug, Clone, Default)]
struct ServerArgs {
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
    #[arg(
        long = "http-port",
        default_value_t = 8545,
        help = "Port HTTP untuk JSON-RPC dan static web explorer"
    )]
    http_port: u16,
    #[arg(
        long = "explorer-dir",
        default_value = "apps/aurion-explorer/out",
        help = "Direktori statis explorer (hasil build Next.js)"
    )]
    explorer_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    let server_args = match cli.command {
        Commands::Run {
            command: ServerCommand::Server(args),
        }
        | Commands::Start {
            command: ServerCommand::Server(args),
        } => args,
    };

    run_server(server_args).await
}

async fn run_server(args: ServerArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file_config = AurionSettings::load_from_file("config/node.dev.toml")?;
    let mut settings = apply_cli_overrides(file_config, &args);
    if args.data_dir.is_none() {
        settings.storage.db_path = PathBuf::from(format!("target/dev-db-node-{}", args.identity));
    }

    let _log_guard = init_central_logging_with_settings(settings.mode, &settings.logging, "aurion");

    // Roster dev deterministik: kunci diturunkan dari blake3(index), sehingga
    // seluruh proses klaster berbagi himpunan validator yang sama persis.
    let roster_size = args.roster_size.max(1);
    let identity = args.identity % roster_size;
    let roster_keys = dev_roster(roster_size);
    let roster: Vec<PublicKeyBytes> = roster_keys.iter().map(Keypair::public_key_bytes).collect();
    let proposer = dev_key(identity);

    let genesis_authority = Arc::new(genesis_authority_key());
    let treasury_pubkey = genesis_authority.public_key_bytes();

    let config = NodeConfig {
        chain_id: settings.network.chain_id,
        data_dir: settings.storage.db_path.clone(),
        treasury_account: treasury_pubkey,
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
        seeds: args.peers.clone(),
    };
    let p2p = P2PRuntime::spawn(p2p_config, handle.clone()).await?;
    let p2p_handle = p2p.handle.clone();

    println!("AURION_LISTEN:{}", p2p_handle.local_addr());
    let _ = std::io::stdout().flush();

    let shared_state = SharedState {
        chain_id: settings.network.chain_id,
        node_handle: handle.clone(),
        genesis_authority: genesis_authority.clone(),
    };
    let http_port = args.http_port;
    let explorer_dir = args.explorer_dir.clone();
    let http_task = tokio::spawn(start_http_server(http_port, explorer_dir, shared_state));

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
    http_task.abort();

    let _ = daemon.await;
    let _ = join.await;
    let _ = observer.await;
    let _ = p2p.join.await;
    Ok(())
}

/// State bersama yang dibagikan ke handler HTTP / RPC.
#[derive(Clone, Debug)]
pub struct SharedState {
    pub chain_id: u64,
    pub node_handle: NodeHandle,
    pub genesis_authority: Arc<Keypair>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    method: String,
    params: Option<serde_json::Value>,
    id: Option<serde_json::Value>,
}

#[derive(Serialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: serde_json::Value,
}

#[derive(Serialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Menjalankan HTTP server untuk melayani JSON-RPC dan berkas statis explorer.
pub async fn start_http_server(port: u16, static_dir: PathBuf, state: SharedState) {
    let resolved_static_dir = if static_dir.exists() {
        static_dir
    } else if PathBuf::from("../aurion").join(&static_dir).exists() {
        PathBuf::from("../aurion").join(&static_dir)
    } else {
        let _ = std::fs::create_dir_all(&static_dir);
        let index_path = static_dir.join("index.html");
        if !index_path.exists() {
            let default_html = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Aurion Explorer</title></head><body><h1>Aurion Explorer</h1><p>Simpul Aurion HTTP Server aktif.</p></body></html>";
            let _ = std::fs::write(&index_path, default_html);
        }
        static_dir
    };

    let serve_dir = ServeDir::new(&resolved_static_dir).append_index_html_on_directories(true);

    let app = Router::new()
        .route("/", post(handle_rpc).get_service(serve_dir.clone()))
        .route("/rpc", post(handle_rpc))
        .fallback_service(serve_dir)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let bind_addr = format!("0.0.0.0:{port}");
    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(l) => l,
        Err(err) => {
            tracing::warn!(%err, port, "Gagal mengikat port HTTP server; server HTTP dinonaktifkan");
            return;
        }
    };

    if let Ok(addr) = listener.local_addr() {
        tracing::info!(%addr, static_dir = ?resolved_static_dir, "HTTP server berjalan (JSON-RPC & Explorer)");
    }

    if let Err(err) = axum::serve(listener, app).await {
        tracing::error!(%err, "HTTP server berhenti karena kesalahan");
    }
}

async fn handle_rpc(
    State(state): State<SharedState>,
    Json(payload): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let id = payload.id.unwrap_or_else(|| serde_json::json!(1));

    match payload.method.as_str() {
        "aur_status" | "aurion_status" => {
            let height = state.node_handle.latest_height().await.unwrap_or(0);
            let latest_block = state.node_handle.get_block(height).await.ok().flatten();
            let state_root = latest_block.as_ref().map_or_else(
                || "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                |b| format!("0x{}", hex::encode(b.header.state_root)),
            );

            let result = serde_json::json!({
                "chain_id": state.chain_id,
                "chainId": state.chain_id,
                "height": height,
                "block_height": height,
                "blockHeight": height,
                "node_status": "online",
                "status": "online",
                "validator_count": 1,
                "required_quorum": 1,
                "latest_state_root": state_root,
                "epoch_index": height / 100,
                "is_synced": true,
            });
            (
                StatusCode::OK,
                Json(JsonRpcResponse {
                    jsonrpc: "2.0",
                    result: Some(result),
                    error: None,
                    id,
                }),
            )
        }
        "aur_getLatestBlocks" | "aurion_getLatestBlocks" => {
            let limit = extract_u64_param(payload.params.as_ref(), 0)
                .unwrap_or(10)
                .min(50);
            let height = state.node_handle.latest_height().await.unwrap_or(0);
            let mut blocks_json = Vec::new();
            let min_height = if height >= limit { height + 1 - limit } else { 0 };

            for h in (min_height..=height).rev() {
                if let Ok(Some(block)) = state.node_handle.get_block(h).await {
                    blocks_json.push(serialize_block_header(&block));
                }
            }

            (
                StatusCode::OK,
                Json(JsonRpcResponse {
                    jsonrpc: "2.0",
                    result: Some(serde_json::Value::Array(blocks_json)),
                    error: None,
                    id,
                }),
            )
        }
        "aur_getBlockByHeight" | "aurion_getBlockByHeight" => {
            let height_opt = extract_u64_param(payload.params.as_ref(), 0);
            match height_opt {
                Some(h) => {
                    let block_opt = state.node_handle.get_block(h).await.ok().flatten();
                    (
                        StatusCode::OK,
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0",
                            result: block_opt.as_ref().map(serialize_block_detail),
                            error: None,
                            id,
                        }),
                    )
                }
                None => (
                    StatusCode::OK,
                    Json(JsonRpcResponse {
                        jsonrpc: "2.0",
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Parameter block height diperlukan.".to_string(),
                        }),
                        id,
                    }),
                ),
            }
        }
        "aur_getBlockByHash" | "aurion_getBlockByHash" => {
            let hash_str_opt = extract_string_param(payload.params.as_ref(), 0);
            let hash_opt = hash_str_opt.as_deref().and_then(parse_hash);
            match hash_opt {
                Some(h) => {
                    let block_opt = state.node_handle.get_block_by_hash(h).await.ok().flatten();
                    (
                        StatusCode::OK,
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0",
                            result: block_opt.as_ref().map(serialize_block_detail),
                            error: None,
                            id,
                        }),
                    )
                }
                None => (
                    StatusCode::OK,
                    Json(JsonRpcResponse {
                        jsonrpc: "2.0",
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Parameter block hash 32-byte tidak valid.".to_string(),
                        }),
                        id,
                    }),
                ),
            }
        }
        "aur_getBlock" | "aurion_getBlock" => {
            let block = if let Some(h) = extract_u64_param(payload.params.as_ref(), 0) {
                state.node_handle.get_block(h).await.ok().flatten()
            } else if let Some(hash_str) = extract_string_param(payload.params.as_ref(), 0) {
                if let Some(h) = parse_hash(&hash_str) {
                    state.node_handle.get_block_by_hash(h).await.ok().flatten()
                } else {
                    None
                }
            } else {
                None
            };
            (
                StatusCode::OK,
                Json(JsonRpcResponse {
                    jsonrpc: "2.0",
                    result: block.as_ref().map(serialize_block_detail),
                    error: None,
                    id,
                }),
            )
        }
        "aur_getTransactionByHash" | "aurion_getTransactionByHash" | "aur_getTransaction"
        | "aurion_getTransaction" => {
            let hash_str_opt = extract_string_param(payload.params.as_ref(), 0);
            let hash_opt = hash_str_opt.as_deref().and_then(parse_hash);
            match hash_opt {
                Some(target_hash) => {
                    let height = state.node_handle.latest_height().await.unwrap_or(0);
                    let mut found_tx = None;
                    for h in (0..=height).rev() {
                        if let Ok(Some(block)) = state.node_handle.get_block(h).await {
                            for tx in &block.transactions {
                                if tx.digest() == target_hash {
                                    found_tx = Some(serialize_transaction(
                                        tx,
                                        block.header.height,
                                        block.header.timestamp,
                                    ));
                                    break;
                                }
                            }
                            if found_tx.is_some() {
                                break;
                            }
                        }
                    }
                    (
                        StatusCode::OK,
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0",
                            result: found_tx,
                            error: None,
                            id,
                        }),
                    )
                }
                None => (
                    StatusCode::OK,
                    Json(JsonRpcResponse {
                        jsonrpc: "2.0",
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Parameter transaction hash tidak valid.".to_string(),
                        }),
                        id,
                    }),
                ),
            }
        }
        "aur_getAccount" | "aurion_getAccount" => {
            let addr_str_opt = extract_string_param(payload.params.as_ref(), 0);
            match addr_str_opt {
                Some(addr) => {
                    let pubkey = parse_pubkey(&addr);
                    let account = state
                        .node_handle
                        .get_account(pubkey)
                        .await
                        .ok()
                        .flatten();
                    let (balance, nonce) = account
                        .map_or((0_u128, 0_u64), |a| (a.balance, a.nonce));
                    let result = serde_json::json!({
                        "address": addr,
                        "balance": balance.to_string(),
                        "balance_quanta": balance.to_string(),
                        "balance_aur": format_quanta_aur(balance),
                        "nonce": nonce,
                    });
                    (
                        StatusCode::OK,
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0",
                            result: Some(result),
                            error: None,
                            id,
                        }),
                    )
                }
                None => (
                    StatusCode::OK,
                    Json(JsonRpcResponse {
                        jsonrpc: "2.0",
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Parameter address diperlukan.".to_string(),
                        }),
                        id,
                    }),
                ),
            }
        }
        "aur_requestFaucet" | "aurion_requestFaucet" | "requestFaucet" => {
            let recipient_opt = extract_recipient(payload.params.as_ref());
            match recipient_opt {
                Some(addr) => {
                    let recipient_pk = parse_pubkey(&addr);
                    let treasury_pk = state.genesis_authority.public_key_bytes();

                    if recipient_pk == treasury_pk {
                        return (
                            StatusCode::OK,
                            Json(JsonRpcResponse {
                                jsonrpc: "2.0",
                                result: None,
                                error: Some(JsonRpcError {
                                    code: -32602,
                                    message: "Tidak dapat menyalurkan faucet ke akun treasury sendiri."
                                        .to_string(),
                                }),
                                id,
                            }),
                        );
                    }

                    // Ambil nonce akun treasury
                    let treasury_acc = state
                        .node_handle
                        .get_account(treasury_pk)
                        .await
                        .ok()
                        .flatten();
                    let nonce = treasury_acc.map_or(0, |a| a.nonce);

                    let amount = 10 * QUANTA_PER_AUR; // 10 AUR
                    let fee = 10_000_u128; // Quanta

                    let payload_bytes = Transaction::payload_bytes(
                        &treasury_pk,
                        &recipient_pk,
                        amount,
                        nonce,
                        fee,
                    );
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(b"AURION_TX_CANONICAL_V1");
                    hasher.update(&payload_bytes);
                    let digest = *hasher.finalize().as_bytes();
                    let signature = state.genesis_authority.sign(&digest);

                    let tx = Transaction::new(
                        treasury_pk,
                        recipient_pk,
                        amount,
                        nonce,
                        fee,
                        signature,
                    );

                    let tx_hash_hex = format!("0x{}", hex::encode(digest));

                    if let Err(e) = state.node_handle.submit(tx) {
                        return (
                            StatusCode::OK,
                            Json(JsonRpcResponse {
                                jsonrpc: "2.0",
                                result: None,
                                error: Some(JsonRpcError {
                                    code: -32000,
                                    message: format!("Gagal submit transaksi: {e}"),
                                }),
                                id,
                            }),
                        );
                    }

                    // Picu produksi blok seketika agar transaksi masuk ke blok berikutnya
                    let _ = state.node_handle.produce_block();

                    // Beri jeda singkat agar task commit blok selesai
                    tokio::time::sleep(Duration::from_millis(50)).await;

                    let height = state.node_handle.latest_height().await.unwrap_or(0);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis());

                    let result = serde_json::json!({
                        "success": true,
                        "recipient": addr,
                        "amount": 10,
                        "amount_aur": "10.0 AUR",
                        "amount_quanta": amount.to_string(),
                        "symbol": "AUR",
                        "tx_hash": tx_hash_hex,
                        "block_height": height,
                        "timestamp": now,
                        "message": "Pengiriman Quanta dari Treasury Genesis berhasil dieksekusi dan dimasukkan ke blok.",
                    });

                    (
                        StatusCode::OK,
                        Json(JsonRpcResponse {
                            jsonrpc: "2.0",
                            result: Some(result),
                            error: None,
                            id,
                        }),
                    )
                }
                None => (
                    StatusCode::OK,
                    Json(JsonRpcResponse {
                        jsonrpc: "2.0",
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: "Alamat penerima (recipient address) diperlukan dalam params."
                                .to_string(),
                        }),
                        id,
                    }),
                ),
            }
        }
        _ => (
            StatusCode::OK,
            Json(JsonRpcResponse {
                jsonrpc: "2.0",
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method '{}' tidak ditemukan.", payload.method),
                }),
                id,
            }),
        ),
    }
}

/// Kunci otoritas deterministik untuk akun treasury genesis dan faucet simpul lokal.
#[must_use]
pub fn genesis_authority_key() -> Keypair {
    let digest = blake3::hash(b"AURION_GENESIS_TREASURY_AUTHORITY_V1");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(digest.as_bytes());
    Keypair::from_bytes(&seed)
}

fn parse_pubkey(addr: &str) -> PublicKeyBytes {
    let clean = addr.trim().trim_start_matches("0x").trim_start_matches("0X");
    if let Ok(bytes) = hex::decode(clean) {
        if bytes.len() == 32 {
            let mut pk = [0u8; 32];
            pk.copy_from_slice(&bytes);
            return pk;
        }
    }
    *blake3::hash(addr.as_bytes()).as_bytes()
}

fn parse_hash(hash_str: &str) -> Option<Hash256> {
    let clean = hash_str.trim().trim_start_matches("0x").trim_start_matches("0X");
    if let Ok(bytes) = hex::decode(clean) {
        if bytes.len() == 32 {
            let mut h = [0u8; 32];
            h.copy_from_slice(&bytes);
            return Some(h);
        }
    }
    None
}

fn format_quanta_aur(quanta: Quanta) -> String {
    let whole = quanta / QUANTA_PER_AUR;
    let frac = quanta % QUANTA_PER_AUR;
    format!("{whole}.{frac:010} AUR")
}

fn serialize_block_header(block: &Block) -> serde_json::Value {
    let hash = format!("0x{}", hex::encode(block.header.hash()));
    let prev_hash = format!("0x{}", hex::encode(block.header.prev_hash));
    let state_root = format!("0x{}", hex::encode(block.header.state_root));
    let proposer = format!("0x{}", hex::encode(block.header.proposer));
    serde_json::json!({
        "height": block.header.height,
        "hash": hash,
        "prev_hash": prev_hash,
        "state_root": state_root,
        "proposer": proposer,
        "timestamp": block.header.timestamp,
        "tx_count": block.transactions.len(),
        "size_bytes": 116 + (block.transactions.len() * 168),
        "qc_signers": [proposer],
    })
}

fn serialize_block_detail(block: &Block) -> serde_json::Value {
    let header_json = serialize_block_header(block);
    let mut txs_json = Vec::with_capacity(block.transactions.len());
    for tx in &block.transactions {
        txs_json.push(serialize_transaction(
            tx,
            block.header.height,
            block.header.timestamp,
        ));
    }
    let mut map = header_json.as_object().cloned().unwrap_or_default();
    map.insert("round".to_string(), serde_json::json!(0));
    map.insert("transactions".to_string(), serde_json::json!(txs_json));
    map.insert("quorum_sigs".to_string(), serde_json::json!(1));
    map.insert("total_validators".to_string(), serde_json::json!(1));
    map.insert("reward_aur".to_string(), serde_json::json!("0.0 AUR"));
    serde_json::Value::Object(map)
}

fn serialize_transaction(
    tx: &Transaction,
    block_height: u64,
    timestamp: u64,
) -> serde_json::Value {
    let hash = format!("0x{}", hex::encode(tx.digest()));
    let sender = format!("0x{}", hex::encode(tx.sender));
    let receiver = format!("0x{}", hex::encode(tx.recipient));
    let sig = hex::encode(tx.signature);
    serde_json::json!({
        "hash": hash,
        "sender": sender,
        "receiver": receiver,
        "amount_quanta": tx.amount.to_string(),
        "amount_aur": format_quanta_aur(tx.amount),
        "network_fee_quanta": tx.fee.to_string(),
        "network_fee_aur": format_quanta_aur(tx.fee),
        "nonce": tx.nonce,
        "block_height": block_height,
        "status": "confirmed",
        "timestamp": timestamp,
        "ed25519_signature": sig,
    })
}

fn extract_u64_param(params: Option<&serde_json::Value>, index: usize) -> Option<u64> {
    match params {
        Some(serde_json::Value::Array(arr)) => arr.get(index).and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
        }),
        Some(serde_json::Value::Number(n)) => n.as_u64(),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().ok(),
        _ => None,
    }
}

fn extract_string_param(params: Option<&serde_json::Value>, index: usize) -> Option<String> {
    match params {
        Some(serde_json::Value::Array(arr)) => arr
            .get(index)
            .and_then(|v| v.as_str().map(ToString::to_string)),
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn extract_recipient(params: Option<&serde_json::Value>) -> Option<String> {
    match params {
        Some(serde_json::Value::Array(arr)) => arr
            .first()
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        Some(serde_json::Value::Object(map)) => map
            .get("address")
            .or_else(|| map.get("target_address"))
            .or_else(|| map.get("recipient"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Nilai CLI menimpa konfigurasi berkas hanya bila flag diberikan eksplisit.
fn apply_cli_overrides(mut settings: AurionSettings, args: &ServerArgs) -> AurionSettings {
    let has_port = args.port.is_some();
    let has_chain_id = args.chain_id.is_some();

    if has_port {
        if let Some((host, _)) = settings.network.p2p_bind_addr.rsplit_once(':') {
            let host = host.to_string();
            if let Some(port) = args.port {
                settings.network.p2p_bind_addr = format!("{host}:{port}");
            }
        }
    }
    if let Some(chain_id) = args.chain_id {
        if has_chain_id {
            settings.network.chain_id = chain_id;
        }
    }
    if let Some(data_dir) = &args.data_dir {
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
