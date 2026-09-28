#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{routing::get, Router};
use clap::Parser;
use tower_http::cors::{Any, CorsLayer};

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
    #[arg(
        short,
        long,
        default_value = "8080",
        help = "Port HTTP untuk Explorer REST API"
    )]
    port: u16,

    #[arg(
        short,
        long,
        default_value = "./data/aurion.db",
        help = "Jalur ke file database redb"
    )]
    data_dir: PathBuf,

    #[arg(short, long, default_value = "1001", help = "Chain ID")]
    chain_id: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    println!("============================================================");
    println!("             AURION BLOCKCHAIN EXPLORER BACKEND             ");
    println!("============================================================");
    println!("Membuka Ledger Store di: {:?}", cli.data_dir);
    let ledger = LedgerStore::open(&cli.data_dir)?;
    println!("Ledger Store berhasil diakses.");
    let shared_state = Arc::new(AppState {
        ledger: Arc::new(ledger),
        chain_id: cli.chain_id,
    });
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let app = Router::new()
        .route("/api/v1/status", get(get_chain_status))
        .route("/api/v1/blocks/:height", get(get_block_by_height))
        .route("/api/v1/accounts/:pubkey_hex", get(get_account))
        .layer(cors)
        .with_state(shared_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port));
    println!(
        "REST API Explorer aktif pada: http://localhost:{}",
        cli.port
    );
    println!("Endpoint yang tersedia:");
    println!("  - GET http://localhost:{}/api/v1/status", cli.port);
    println!(
        "  - GET http://localhost:{}/api/v1/blocks/:height",
        cli.port
    );
    println!(
        "  - GET http://localhost:{}/api/v1/accounts/:pubkey_hex",
        cli.port
    );
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
