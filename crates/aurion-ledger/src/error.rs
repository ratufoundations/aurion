use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

// redb error types disimpan sebagai String agar LedgerError tetap kecil
// (clippy::result_large_err) tanpa perlu #[allow(...)].
#[derive(Error, Debug)]
pub enum LedgerError {
    #[error("Database disk error: {0}")]
    DatabaseError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Table error: {0}")]
    TableError(String),

    #[error("Commit error: {0}")]
    CommitError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Data biner blok korup pada tinggi {0}")]
    CorruptedBlock(u64),

    #[error("Data biner transaksi tidak valid: panjang diharapkan {expected}, diterima {got}")]
    InvalidTransactionLength { expected: usize, got: usize },

    #[error("Data biner ledger tidak valid")]
    MalformedData,

    #[error("Akun {0:?} tidak ditemukan di ledger")]
    AccountNotFound(PublicKeyBytes),

    #[error("Blok pada tinggi {0} tidak ditemukan")]
    BlockNotFound(u64),

    #[error("Blok pada tinggi {0} sudah ada dan bersifat append-only")]
    BlockAlreadyExists(u64),

    #[error("Tinggi blok tidak berurutan: diharapkan {expected}, diterima {actual}")]
    NonSequentialBlock { expected: u64, actual: u64 },

    #[error("Tinggi blok telah mencapai nilai maksimum")]
    HeightOverflow,

    #[error("Hash parent blok pada tinggi {0} tidak cocok dengan tip ledger")]
    PreviousHashMismatch(u64),

    #[error("Timestamp blok tidak monotonik terhadap parent")]
    TimestampNotMonotonic,

    #[error("Eksekusi blok gagal: {0}")]
    ExecutionFailed(#[from] aurion_core::ExecutionError),
}
