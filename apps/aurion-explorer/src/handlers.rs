use crate::dto::{AccountDto, ApiResponse, BlockDto, ChainStatusDto, TransactionDto};
use aurion_ledger::LedgerStore;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct AppState {
    pub(crate) ledger: Arc<LedgerStore>,
    pub(crate) chain_id: u64,
}

/// GET /api/v1/status - Ringkasan status rantai
pub(crate) async fn get_chain_status(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<ApiResponse<ChainStatusDto>>) {
    let latest_height = state.ledger.get_latest_height().unwrap_or(0);
    let (latest_block_hash, latest_state_root) = if latest_height > 0 {
        match state.ledger.get_block_by_height(latest_height) {
            Ok(Some(block)) => (
                hex::encode(block.header.hash()),
                hex::encode(block.header.state_root),
            ),
            _ => (str::repeat("0", 64), str::repeat("0", 64)),
        }
    } else {
        (str::repeat("0", 64), str::repeat("0", 64))
    };
    let server_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    (
        StatusCode::OK,
        Json(ApiResponse::ok(ChainStatusDto {
            chain_id: state.chain_id,
            latest_height,
            latest_block_hash,
            latest_state_root,
            server_time,
        })),
    )
}

/// GET /api/v1/blocks/:height - Ambil detail blok berdasarkan tinggi
pub(crate) async fn get_block_by_height(
    State(state): State<Arc<AppState>>,
    Path(height): Path<u64>,
) -> (StatusCode, Json<ApiResponse<BlockDto>>) {
    match state.ledger.get_block_by_height(height) {
        Ok(Some(block)) => {
            let txs = block
                .transactions
                .iter()
                .map(|tx| TransactionDto {
                    sender: hex::encode(tx.sender),
                    recipient: hex::encode(tx.recipient),
                    amount: tx.amount,
                    nonce: tx.nonce,
                    signature: hex::encode(tx.signature),
                    tx_hash: hex::encode(tx.digest()),
                })
                .collect();
            let dto = BlockDto {
                height: block.header.height,
                block_hash: hex::encode(block.header.hash()),
                prev_hash: hex::encode(block.header.prev_hash),
                state_root: hex::encode(block.header.state_root),
                tx_count: block.header.tx_count,
                transactions: txs,
            };
            (StatusCode::OK, Json(ApiResponse::ok(dto)))
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err(format!(
                "Blok tinggi {height} tidak ditemukan"
            ))),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(format!("Kesalahan pembacaan ledger: {e}"))),
        ),
    }
}

/// GET /`api/v1/accounts/:pubkey_hex` - Saldo dan Nonce akun
pub(crate) async fn get_account(
    State(state): State<Arc<AppState>>,
    Path(pubkey_hex): Path<String>,
) -> (StatusCode, Json<ApiResponse<AccountDto>>) {
    let pubkey_bytes = match hex::decode(&pubkey_hex) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            arr
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::err(
                    "Format public key hex tidak valid (harus 64 karakter hex)",
                )),
            );
        }
    };
    match state.ledger.get_account(&pubkey_bytes) {
        Ok(Some(acc)) => (
            StatusCode::OK,
            Json(ApiResponse::ok(AccountDto {
                address: pubkey_hex,
                balance: acc.balance,
                nonce: acc.nonce,
            })),
        ),
        Ok(None) => (
            StatusCode::OK,
            Json(ApiResponse::ok(AccountDto {
                address: pubkey_hex,
                balance: 0,
                nonce: 0,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(format!(
                "Kesalahan pembacaan database: {e}"
            ))),
        ),
    }
}
