use std::sync::Arc;
use tokio::sync::Mutex;
use zenoh::Session;

use aurion_core::{Block, State};
use aurion_ledger::{Codec, LedgerStore};
use aurion_mempool::Mempool;

use crate::{error::GatewayError, routes::GatewayRoutes};

#[derive(Debug)]
pub struct AurionGateway {
    pub chain_id: u64,
    pub session: Arc<Session>,
    pub ledger: Arc<LedgerStore>,
    pub mempool: Arc<Mutex<Mempool>>,
}

impl AurionGateway {
    /// Inisialisasi Sesi Zenoh Gateway.
    ///
    /// # Errors
    /// Mengembalikan error bila sesi Zenoh atau queryable gagal dibuat.
    pub async fn start(
        chain_id: u64,
        ledger: Arc<LedgerStore>,
        mempool: Arc<Mutex<Mempool>>,
    ) -> Result<Self, GatewayError> {
        let config = zenoh::Config::default();
        let session = Arc::new(zenoh::open(config).await?);

        tracing::info!(router_id = %session.zid(), "Sesi Zenoh gateway aktif");

        let gateway = Self {
            chain_id,
            session,
            ledger,
            mempool,
        };

        gateway.register_queryables().await?;

        Ok(gateway)
    }

    /// Registrasi Queryable RPC untuk Status Rantai, Akun, dan Submit Tx
    async fn register_queryables(&self) -> Result<(), GatewayError> {
        let chain_id = self.chain_id;

        // 1. QUERYABLE: Status Rantai (aurion/{chain_id}/status)
        let status_key = GatewayRoutes::status(chain_id);
        let status_queryable = self.session.declare_queryable(&status_key).await?;
        let ledger_for_status = Arc::clone(&self.ledger);

        tokio::spawn(async move {
            while let Ok(query) = status_queryable.recv_async().await {
                tracing::debug!(topic = %query.key_expr(), "Menerima kueri Zenoh dari klien eksternal");
                let height = ledger_for_status.get_latest_height().unwrap_or(0);
                let payload = serde_json::json!({
                    "chain_id": chain_id,
                    "latest_height": height,
                })
                .to_string();

                if let Err(e) = query.reply(query.key_expr(), payload).await {
                    tracing::error!(error = %e, "Gagal membalas kueri status Zenoh");
                }
            }
        });

        // 2. QUERYABLE: Pengecekan Akun (aurion/{chain_id}/account/*)
        let account_pattern = GatewayRoutes::account_wildcard(chain_id);
        let account_queryable = self.session.declare_queryable(&account_pattern).await?;
        let ledger_for_account = Arc::clone(&self.ledger);

        tokio::spawn(async move {
            while let Ok(query) = account_queryable.recv_async().await {
                tracing::debug!(topic = %query.key_expr(), "Menerima kueri Zenoh dari klien eksternal");
                let key_str = query.key_expr().as_str();
                // Ambil segmen terakhir sebagai pubkey_hex
                if let Some(pubkey_hex) = key_str.split('/').next_back() {
                    let response_payload = match hex::decode(pubkey_hex) {
                        Ok(bytes) if bytes.len() == 32 => {
                            let mut pk = [0u8; 32];
                            pk.copy_from_slice(&bytes);
                            let (balance, nonce) = ledger_for_account
                                .get_account(&pk)
                                .ok()
                                .flatten()
                                .map_or((0, 0), |acc| (acc.balance, acc.nonce));

                            serde_json::json!({
                                "address": pubkey_hex,
                                "balance": balance,
                                "nonce": nonce,
                            })
                        }
                        _ => serde_json::json!({ "error": "Kunci publik hex tidak valid" }),
                    };

                    let _ = query
                        .reply(query.key_expr(), response_payload.to_string())
                        .await;
                }
            }
        });

        // 3. QUERYABLE: Kirim Transaksi Mentah (aurion/{chain_id}/tx/submit)
        let tx_key = GatewayRoutes::tx_submit(chain_id);
        let tx_queryable = self.session.declare_queryable(&tx_key).await?;
        let mempool_for_tx = Arc::clone(&self.mempool);

        tokio::spawn(async move {
            while let Ok(query) = tx_queryable.recv_async().await {
                tracing::debug!(topic = %query.key_expr(), "Menerima kueri Zenoh dari klien eksternal");
                let reply_payload = if let Some(payload) = query.payload() {
                    let raw_bytes = payload.to_bytes();
                    match Codec::decode_tx(&raw_bytes) {
                        Ok(tx) => {
                            let tx_hash = hex::encode(tx.digest());
                            let mut mp = mempool_for_tx.lock().await;
                            let dummy_state = State::new();

                            match mp.insert(tx, 0, &dummy_state) {
                                Ok(()) => {
                                    tracing::info!(tx_hash = %tx_hash, "Transaksi gateway diterima mempool");
                                    serde_json::json!({
                                        "success": true,
                                        "tx_hash": tx_hash
                                    })
                                }
                                Err(e) => {
                                    tracing::warn!(tx_hash = %tx_hash, error = %e, "Mempool menolak transaksi gateway");
                                    serde_json::json!({
                                        "success": false,
                                        "error": format!("Mempool ditolak: {}", e)
                                    })
                                }
                            }
                        }
                        Err(e) => serde_json::json!({
                            "success": false,
                            "error": format!("Format transaksi korup: {}", e)
                        }),
                    }
                } else {
                    serde_json::json!({ "success": false, "error": "Payload transaksi kosong" })
                };

                let _ = query
                    .reply(query.key_expr(), reply_payload.to_string())
                    .await;
            }
        });

        Ok(())
    }

    /// Siarkan blok baru secara real-time ke semua pelanggan Zenoh.
    ///
    /// # Errors
    /// Mengembalikan error bila pengiriman ke sesi Zenoh gagal.
    pub async fn broadcast_block(&self, block: &Block) -> Result<(), GatewayError> {
        let topic = GatewayRoutes::events_blocks(self.chain_id);
        let block_bytes = Codec::encode_block(block);

        self.session.put(&topic, block_bytes).await?;
        tracing::debug!(topic = %topic, height = block.header.height, "Blok disiarkan melalui Zenoh");

        Ok(())
    }
}
