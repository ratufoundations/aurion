#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{routing::get, Router};
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};

use aurion_config::{init_central_logging_with_settings, AurionSettings};
use aurion_ledger::LedgerStore;

mod dto;
mod handlers;

use handlers::{get_account, get_block_by_height, get_chain_status, AppState};

#[derive(Parser, Debug)]
#[command(
    name = "aurion-explorer",
    version = "0.1.0",
    about = "Backend API Explorer untuk Aurion Blockchain"
)]
struct Cli {
    #[arg(short, long, help = "Port HTTP untuk Explorer REST API")]
    port: Option<u16>,

    #[arg(short, long, help = "Jalur ke file database redb")]
    data_dir: Option<PathBuf>,

    #[arg(short, long, help = "Chain ID")]
    chain_id: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut settings = AurionSettings::load_from_file("config/node.dev.toml")?;
    if let Some(port) = cli.port {
        let mut http_addr: SocketAddr = settings.explorer.http_bind_addr.parse()?;
        http_addr.set_port(port);
        settings.explorer.http_bind_addr = http_addr.to_string();
    }
    if let Some(chain_id) = cli.chain_id {
        settings.network.chain_id = chain_id;
    }
    if let Some(data_dir) = cli.data_dir {
        settings.storage.db_path = data_dir;
    }
    let _guard =
        init_central_logging_with_settings(settings.mode, &settings.logging, "aurion-explorer");
    tracing::info!(mode = ?settings.mode, "Aurion Explorer API daemon mulai");
    tracing::info!(db_path = %settings.storage.db_path.display(), "Membuka Ledger Store");
    let ledger = LedgerStore::open(&settings.storage.db_path)?;
    let shared_state = Arc::new(AppState {
        ledger: Arc::new(ledger),
        chain_id: settings.network.chain_id,
    });
    let mut cors = CorsLayer::new().allow_methods(Any).allow_headers(Any);
    if settings.explorer.cors_enabled {
        cors = cors.allow_origin(Any);
    }
    let app = Router::new()
        .route("/api/v1/status", get(get_chain_status))
        .route("/api/v1/blocks/:height", get(get_block_by_height))
        .route("/api/v1/accounts/:pubkey_hex", get(get_account))
        .layer(cors)
        .with_state(shared_state);
    let addr: SocketAddr = settings.explorer.http_bind_addr.parse()?;
    tracing::info!(%addr, "REST API Explorer mendengarkan");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
