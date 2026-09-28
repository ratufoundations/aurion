use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ExecutionError {
    #[error("Kunci publik pengirim tidak terdaftar di State")]
    AccountNotFound(PublicKeyBytes),

    #[error("Saldo tidak mencukupi: tersedia {available}, dibutuhkan {required}")]
    InsufficientBalance { available: u64, required: u64 },

    #[error("Nonce transaksi tidak cocok: diharapkan {expected}, diterima {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("Transfer ke diri sendiri dilarang")]
    SelfTransferForbidden,

    #[error("Terjadi luapan aritmatika (arithmetic overflow)")]
    ArithmeticOverflow,

    #[error("Tanda tangan kriptografi transaksi tidak sah")]
    InvalidSignature,

    #[error("State Root tidak sesuai dengan komputasi")]
    StateRootMismatch,
}
