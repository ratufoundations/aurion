#![forbid(unsafe_code)]

use std::error::Error;

use aurion_config::{init_central_logging_with_settings, AppMode, AurionSettings};
use aurion_criptografi::Keypair;
use aurion_gateway::GatewayRoutes;
use aurion_ledger::Codec;
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};

#[derive(Parser, Debug)]
#[command(
    name = "aurion-cli",
    version = "0.1.0",
    about = "Terminal Klien & Utilitas Administrasi Aurion Blockchain",
    long_about = "CLI resmi untuk membuat kredensial, memeriksa status konsensus, dan memantau siaran blok Aurion via Zenoh."
)]
struct Cli {
    #[arg(short, long, help = "ID Jaringan Aurion (Chain ID)")]
    chain_id: Option<u64>,

    #[arg(short, long, help = "Aktifkan log terperinci di terminal")]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Buat pasangan kunci kriptografi Ed25519 baru (Offline)
    Keygen,

    /// Ambil status terkini dari simpul jaringan
    Status,

    /// Periksa saldo akun berdasarkan kunci publik (Hex 64-karakter)
    Balance {
        #[arg(help = "Kunci publik 32-byte dalam representasi hex")]
        address: String,
    },

    /// Pantau produksi blok baru secara real-time (Live Stream)
    Monitor,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();
    let config = AurionSettings::load_from_file("config/node.dev.toml")?;
    let chain_id = cli.chain_id.unwrap_or(config.network.chain_id);
    let mode = if cli.verbose {
        AppMode::Developer
    } else {
        AppMode::Production
    };
    let mut logging = config.logging.clone();
    logging.log_to_stdout &= cli.verbose;
    let _guard = init_central_logging_with_settings(mode, &logging, "aurion-cli");
    tracing::info!(chain_id, verbose = cli.verbose, "Aurion CLI mulai");

    match cli.command {
        Commands::Keygen => {
            cmd_keygen();
            Ok(())
        }
        Commands::Status => cmd_status(chain_id).await,
        Commands::Balance { address } => cmd_balance(chain_id, &address).await,
        Commands::Monitor => cmd_monitor(chain_id).await,
    }
}

/// 1. Perintah Keygen (Berjalan offline murni, tidak butuh koneksi jaringan)
fn cmd_keygen() {
    let keypair = Keypair::generate();
    let pub_bytes = keypair.public_key_bytes();
    let priv_bytes = keypair.private_key_bytes();

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("KREDENSIAL KRIPTOGRAFI BARU (Ed25519)").fg(Color::Cyan),
        Cell::new("NILAI BINER (HEX)").fg(Color::Cyan),
    ]);

    table.add_row(vec![
        Cell::new("Public Key (Alamat Akun)").fg(Color::Green),
        Cell::new(hex::encode(pub_bytes)),
    ]);
    table.add_row(vec![
        Cell::new("Private Key (Kunci Rahasia)").fg(Color::Red),
        Cell::new(hex::encode(priv_bytes)),
    ]);

    println!("{table}");
    println!(
        "PERINGATAN: Simpan kunci rahasia Anda di tempat aman. Jangan bagikan kepada siapa pun."
    );
}

/// 2. Perintah Status (Kueri ke Zenoh Queryable: `aurion/{chain_id}/status`)
async fn cmd_status(chain_id: u64) -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Membuka sesi Zenoh...");
    let session = zenoh::open(zenoh::Config::default()).await?;
    let target_key = GatewayRoutes::status(chain_id);

    println!("Mengambil status rantai dari: {target_key} ...");
    let replies = session.get(&target_key).await?;

    let mut found = false;
    while let Ok(reply) = replies.recv_async().await {
        match reply.result() {
            Ok(sample) => {
                let payload = sample.payload().to_bytes();
                let payload_str = std::str::from_utf8(payload.as_ref())?;
                let v: serde_json::Value = serde_json::from_str(payload_str)?;

                let mut table = Table::new();
                table.load_preset(UTF8_FULL);
                table.set_header(vec![
                    Cell::new("PARAMETER JARINGAN").fg(Color::Cyan),
                    Cell::new("NILAI STATUS").fg(Color::Cyan),
                ]);

                table.add_row(vec![
                    Cell::new("Chain ID"),
                    Cell::new(v["chain_id"].to_string()).fg(Color::Yellow),
                ]);
                table.add_row(vec![
                    Cell::new("Tinggi Blok Terkini (Height)"),
                    Cell::new(v["latest_height"].to_string()).fg(Color::Green),
                ]);

                println!("{table}");
                found = true;
                break;
            }
            Err(e) => {
                eprintln!("Respons galat dari jaringan: {e:?}");
            }
        }
    }

    if !found {
        eprintln!("Tidak ada respons dari simpul Aurion. Pastikan aurion-node sedang berjalan.");
    }

    session.close().await?;
    Ok(())
}

/// Konstanta konversi unit saldo: 1 AUR = 1.000.000 Quanta.
const QUANTA_PER_AUR: u64 = 1_000_000;

/// 3. Perintah Balance (Zero-float: kalkulasi konversi Quanta ke AUR via modulus)
async fn cmd_balance(chain_id: u64, address: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    if address.len() != 64 {
        eprintln!("Format alamat tidak valid: panjang hex harus tepat 64 karakter (32 byte).");
        return Ok(());
    }

    let session = zenoh::open(zenoh::Config::default()).await?;
    let target_key = GatewayRoutes::account_exact(chain_id, address);

    println!("Memeriksa saldo ke: {target_key} ...");
    let replies = session.get(&target_key).await?;

    let mut found = false;
    while let Ok(reply) = replies.recv_async().await {
        match reply.result() {
            Ok(sample) => {
                let payload = sample.payload().to_bytes();
                let payload_str = std::str::from_utf8(payload.as_ref())?;
                let v: serde_json::Value = serde_json::from_str(payload_str)?;

                if let Some(err) = v.get("error") {
                    eprintln!("Kesalahan: {err}");
                    return Ok(());
                }

                let balance_quanta = v["balance"].as_u64().unwrap_or(0);
                let nonce = v["nonce"].as_u64().unwrap_or(0);

                // Perhitungan fixed-point tanpa float
                let whole_aur = balance_quanta / QUANTA_PER_AUR;
                let remainder_quanta = balance_quanta % QUANTA_PER_AUR;
                let formatted_aur = format!("{whole_aur}.{remainder_quanta:06} AUR");

                let mut table = Table::new();
                table.load_preset(UTF8_FULL);
                table.set_header(vec![
                    Cell::new("ATRIBUT AKUN").fg(Color::Cyan),
                    Cell::new("NILAI AKUN").fg(Color::Cyan),
                ]);

                table.add_row(vec![Cell::new("Alamat (Public Key)"), Cell::new(address)]);
                table.add_row(vec![
                    Cell::new("Saldo (Quanta)"),
                    Cell::new(balance_quanta.to_string()),
                ]);
                table.add_row(vec![
                    Cell::new("Saldo (AUR)"),
                    Cell::new(formatted_aur).fg(Color::Green),
                ]);
                table.add_row(vec![
                    Cell::new("Nonce Transaksi"),
                    Cell::new(nonce.to_string()),
                ]);

                println!("{table}");
                found = true;
                break;
            }
            Err(e) => {
                eprintln!("Respons galat dari jaringan: {e:?}");
            }
        }
    }

    if !found {
        eprintln!("Tidak ada respons dari simpul Aurion.");
    }

    session.close().await?;
    Ok(())
}

/// 4. Perintah Monitor (Langganan ke Zenoh Publisher: `aurion/{chain_id}/events/blocks`)
async fn cmd_monitor(chain_id: u64) -> Result<(), Box<dyn Error + Send + Sync>> {
    let session = zenoh::open(zenoh::Config::default()).await?;
    let topic = GatewayRoutes::events_blocks(chain_id);

    let subscriber = session.declare_subscriber(&topic).await?;
    println!("Mendengarkan siaran blok baru pada topik: {topic}");
    println!("Tekan Ctrl+C untuk menghentikan pemantauan.\n");

    while let Ok(sample) = subscriber.recv_async().await {
        let block_bytes = sample.payload().to_bytes();
        match Codec::decode_block(block_bytes.as_ref()) {
            Ok(block) => {
                let state_root_short = hex::encode(&block.header.state_root[0..4]);
                let prev_hash_short = hex::encode(&block.header.prev_hash[0..4]);

                println!(
                    "[BLOK DITERIMA] Height: {:<6} | Tx: {:<3} | State Root: {state_root_short}... | Prev: {prev_hash_short}...",
                    block.header.height,
                    block.header.tx_count,
                );
            }
            Err(e) => {
                eprintln!("Gagal mengurai blok masuk: {e}");
            }
        }
    }

    Ok(())
}
