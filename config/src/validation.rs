#![forbid(unsafe_code)]

use crate::settings::AurionSettings;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Chain ID tidak boleh 0")]
    InvalidChainId,
    #[error("Interval waktu blok terlalu cepat: {0} ms (minimal 500 ms)")]
    BlockTimeTooFast(u64),
    #[error("Batas kuorum persentase wajib antara 51 dan 100, terkonfigurasi: {0}")]
    InvalidQuorum(u64),
    #[error("Batas transaksi per blok tidak boleh 0")]
    ZeroTxPerBlock,
}

/// Memvalidasi invarian konfigurasi sebelum aplikasi dijalankan.
///
/// # Errors
/// Mengembalikan error bila chain ID, waktu blok, kuorum, atau batas transaksi tidak valid.
pub fn validate_settings(settings: &AurionSettings) -> Result<(), ValidationError> {
    if settings.network.chain_id == 0 {
        return Err(ValidationError::InvalidChainId);
    }
    if settings.consensus.block_time_ms < 500 {
        return Err(ValidationError::BlockTimeTooFast(
            settings.consensus.block_time_ms,
        ));
    }
    let quorum = settings.consensus.quorum_threshold_percent;
    if !(51..=100).contains(&quorum) {
        return Err(ValidationError::InvalidQuorum(quorum));
    }
    if settings.consensus.max_tx_per_block == 0 {
        return Err(ValidationError::ZeroTxPerBlock);
    }
    Ok(())
}
