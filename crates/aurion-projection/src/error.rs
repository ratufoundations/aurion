//! Tipe galat terstruktur untuk mesin proyeksi model baca CQRS.
//!
//! Seluruh jalur produksi modul ini mengembalikan [`ProjectionError`] bertipe
//! kuat; tidak ada `unwrap`/`expect` dan tidak ada panic di jalur ingesti,
//! pembacaan model baca, pembersihan, maupun rekonstruksi dingin.

use thiserror::Error;

use aurion_criptografi::Hash256 as AccountId;

/// Galat terstruktur subsistem proyeksi (`aurion-projection`).
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    // ========================================================================
    // [P0] Konsumsi Delta & Idempotensi Proyeksi
    // ========================================================================
    #[error("Ketinggian blok tidak berurutan: diharapkan {expected}, diterima {got}")]
    NonSequentialBlock { expected: u64, got: u64 },

    #[error("Blok pada tinggi {height} sudah diproyeksikan; delta duplikat ditolak")]
    BlockAlreadyProjected { height: u64 },

    #[error("Kursor proyeksi belum diinisialisasi; tinggi saat ini {height}")]
    CursorNotInitialized { height: u64 },

    #[error("Ketinggian blok mencapai batas maksimum u64")]
    HeightOverflow,

    #[error("Kunci delta berada di luar namespace proyeksi: {key:?}")]
    DeltaKeyOutOfNamespace { key: String },

    #[error("Nilai akun pada delta tidak valid: {reason}")]
    MalformedDeltaValue { reason: String },

    #[error("Urutan entri indeks transaksi tidak monotonik: {reason}")]
    OutOfOrderIndexEntry { reason: String },

    // ========================================================================
    // [P1] Indeksasi Riwayat Transaksi Berbasis Alamat
    // ========================================================================
    #[error("Batas paginasi tidak valid: limit {limit} di luar rentang 1..={max}")]
    InvalidPaginationLimit { limit: u64, max: u64 },

    #[error("Offset paginasi tidak valid: {offset} (luap akumulasi dengan limit)")]
    InvalidPaginationOffset { offset: u64 },

    // ========================================================================
    // [P2] Konsistensi Snapshot Model Baca & Deteksi Lag
    // ========================================================================
    #[error("State root proyeksi tidak cocok pada tinggi {height}: diharapkan {expected}, ditemukan {got}")]
    StateRootMismatch {
        height: u64,
        expected: String,
        got: String,
    },

    #[error("Snapshot pada tinggi {height} tidak tersedia dalam basis data proyeksi")]
    SnapshotNotFound { height: u64 },

    #[error("Ketinggian ledger {ledger_height} tertinggal di belakang ketinggian proyeksi {projection_height}")]
    LedgerHeightLags {
        ledger_height: u64,
        projection_height: u64,
    },

    // ========================================================================
    // [P3] Batas Retensi & Pembersihan Aman Histori Usang
    // ========================================================================
    #[error("Jendela retensi tidak valid: {window} (harus lebih besar dari nol)")]
    InvalidRetentionWindow { window: u64 },

    #[error("Tinggi pembersihan {prune_height} melampaui kursor proyeksi {cursor}")]
    PruneHeightExceedsCursor { prune_height: u64, cursor: u64 },

    #[error("Tidak ada histori yang memenuhi syarat dipangkas pada tinggi {height}")]
    NothingToPrune { height: u64 },

    // ========================================================================
    // [P4] Pemulihan Kerusakan & Rekonstruksi Dingin Deterministik
    // ========================================================================
    #[error("Data blok tidak tersedia pada tinggi {height}")]
    BlockUnavailable { height: u64 },

    #[error("Sumber blok melaporkan tinggi {reported}, bukan {expected}")]
    UnexpectedLatestHeight { reported: u64, expected: u64 },

    #[error("Digest rekonstruksi tidak cocok: diharapkan {expected}, ditemukan {got}")]
    DigestMismatch { expected: String, got: String },

    // ========================================================================
    // [P5] Metrik Kinerja & Rasio Kuota Nir-Pecahan
    // ========================================================================
    #[error("Akumulasi metrik meluap pada field {field}")]
    MetricsOverflow { field: String },

    // ========================================================================
    // Sistem & Integrasi
    // ========================================================================
    #[error("Kunci baca-tulis gagal untuk {context} akibat panik pada thread sebelumnya")]
    LockPoisoned { context: &'static str },

    #[error("Galat ledger: {0}")]
    Ledger(String),

    #[error("Galat eksekusi: {0}")]
    Execution(#[from] aurion_core::ExecutionError),
}

impl ProjectionError {
    /// whether [`Self::is_ingestion_error`] galat berasal dari guard ingestion P0.
    #[must_use]
    pub fn is_ingestion_error(&self) -> bool {
        matches!(
            self,
            Self::NonSequentialBlock { .. }
                | Self::BlockAlreadyProjected { .. }
                | Self::CursorNotInitialized { .. }
                | Self::HeightOverflow
        )
    }

    /// whether [`Self::is_indexing_error`] galat berasal dari paginasi/indeks P1.
    #[must_use]
    pub fn is_indexing_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidPaginationLimit { .. }
                | Self::InvalidPaginationOffset { .. }
                | Self::OutOfOrderIndexEntry { .. }
        )
    }

    /// whether [`Self::is_snapshot_error`] galat berasal dari snapshot/lag P2.
    #[must_use]
    pub fn is_snapshot_error(&self) -> bool {
        matches!(
            self,
            Self::StateRootMismatch { .. }
                | Self::SnapshotNotFound { .. }
                | Self::LedgerHeightLags { .. }
        )
    }

    /// whether [`Self::is_pruning_error`] galat berasal dari retensi/pemangkasan P3.
    #[must_use]
    pub fn is_pruning_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidRetentionWindow { .. }
                | Self::PruneHeightExceedsCursor { .. }
                | Self::NothingToPrune { .. }
        )
    }

    /// whether [`Self::is_recovery_error`] galat berasal dari rekonstruksi dingin P4.
    #[must_use]
    pub fn is_recovery_error(&self) -> bool {
        matches!(
            self,
            Self::BlockUnavailable { .. }
                | Self::UnexpectedLatestHeight { .. }
                | Self::DigestMismatch { .. }
        )
    }

    /// whether [`Self::is_metrics_error`] galat berasal dari akuntansi metrik P5.
    #[must_use]
    pub fn is_metrics_error(&self) -> bool {
        matches!(self, Self::MetricsOverflow { .. })
    }

    /// Whether [`Self::is_ingestion_error`] helper did not classify this error.
    ///
    /// Convenience untuk diagnostik: `true` bila galat tidak terjadi pada jalur
    /// ingestion delta.
    #[must_use]
    pub fn is_non_ingestion_error(&self) -> bool {
        !self.is_ingestion_error()
    }

    /// Bentuk kanonik (heksadesimal) dari sebuah `AccountId` untuk pesan galat.
    #[must_use]
    pub fn describe_account(account: &AccountId) -> String {
        let mut out = String::with_capacity(64);
        for byte in account {
            use std::fmt::Write;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_ingestion_error_classification() {
        let err = ProjectionError::NonSequentialBlock {
            expected: 2,
            got: 3,
        };
        assert!(err.is_ingestion_error());
        assert!(!err.is_non_ingestion_error());
        assert!(!err.is_indexing_error());
    }

    #[test]
    fn test_duplicate_block_classification() {
        let err = ProjectionError::BlockAlreadyProjected { height: 2 };
        assert!(err.is_ingestion_error());
    }

    #[test]
    fn test_indexing_error_classification() {
        let err = ProjectionError::InvalidPaginationLimit { limit: 0, max: 100 };
        assert!(err.is_indexing_error());
        assert!(!err.is_ingestion_error());
        assert!(err.is_non_ingestion_error());
    }

    #[test]
    fn test_snapshot_error_classification() {
        let err = ProjectionError::LedgerHeightLags {
            ledger_height: 90,
            projection_height: 95,
        };
        assert!(err.is_snapshot_error());
    }

    #[test]
    fn test_pruning_error_classification() {
        let err = ProjectionError::PruneHeightExceedsCursor {
            prune_height: 2000,
            cursor: 1500,
        };
        assert!(err.is_pruning_error());
    }

    #[test]
    fn test_recovery_error_classification() {
        let err = ProjectionError::BlockUnavailable { height: 42 };
        assert!(err.is_recovery_error());
    }

    #[test]
    fn test_metrics_error_classification() {
        let err = ProjectionError::MetricsOverflow {
            field: "cache_hits".to_owned(),
        };
        assert!(err.is_metrics_error());
        assert!(!err.is_ingestion_error());
    }

    #[test]
    fn test_describe_account_is_hex() {
        let account: AccountId = [0xab; 32];
        let described = ProjectionError::describe_account(&account);
        assert_eq!(described.len(), 64);
        assert!(described.starts_with("abab"));
    }

    #[test]
    fn test_error_display_messages_are_stable() {
        let err = ProjectionError::NonSequentialBlock {
            expected: 2,
            got: 3,
        };
        assert_eq!(
            err.to_string(),
            "Ketinggian blok tidak berurutan: diharapkan 2, diterima 3"
        );
    }
}
