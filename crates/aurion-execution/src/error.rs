use crate::capability::ModuleId;
use thiserror::Error;

/// Tipe galat terstruktur untuk subsistem eksekusi Aurion.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ExecutionError {
    #[error("Akses penyimpanan lintas partisi ditolak: keeper '{holder}' mencoba mengakses partisi {attempted_prefix:?}")]
    UnauthorizedStoreAccess {
        holder: String,
        attempted_prefix: [u8; 4],
    },

    #[error("Eksekusi transaksi dibatalkan (rollback): {cause}")]
    ExecutionReverted { cause: Box<ExecutionError> },

    #[error("Kapabilitas '{capability}' tidak dipegang oleh modul {grantee:?}")]
    CapabilityMissing {
        capability: String,
        grantee: ModuleId,
    },

    #[error("Fuel transaksi habis: dibutuhkan {required}, tersisa {remaining}")]
    OutOfFuel { required: u64, remaining: u64 },

    #[error("Kapabilitas '{capability}' milik {grantee:?} kedaluwarsa pada blok {expired_at} (blok saat ini {current_block})")]
    CapabilityExpired {
        capability: String,
        grantee: ModuleId,
        expired_at: u64,
        current_block: u64,
    },

    #[error("Format StoreKey tidak valid: {reason}")]
    InvalidStoreKey { reason: &'static str },

    #[error("State corrupt atau format nilai tidak valid: {reason}")]
    MalformedState { reason: &'static str },

    #[error("Saldo tidak mencukupi: tersedia {available}, dibutuhkan {required}")]
    InsufficientBalance { available: u64, required: u64 },

    #[error("Transfer ke alamat terlarang ditolak: {0:?}")]
    ForbiddenRecipient([u8; 32]),

    #[error("Transfer ke alamat pengirim sendiri dilarang")]
    SelfTransferForbidden,

    #[error("Aritmetika Quanta meluap (overflow/underflow)")]
    ArithmeticOverflow,

    #[error("Tanda tangan amplop transaksi tidak valid")]
    InvalidSignature,

    #[error("Chain ID amplop tidak cocok: diharapkan {expected}, diterima {got}")]
    InvalidChainId { expected: u64, got: u64 },

    #[error("Aksi {action} tidak dikenal untuk modul {module}")]
    UnknownAction { module: String, action: u16 },

    #[error("Kode aksi {0} tidak dikenal")]
    UnknownActionCode(u16),

    #[error("Jumlah aksi melebihi batas kebijakan: {count} > {limit}")]
    TooManyActions { count: usize, limit: usize },

    #[error("Kegagalan yang dipicu secara eksplisit untuk pengujian rollback: {reason}")]
    ExplicitFailure { reason: String },
}
