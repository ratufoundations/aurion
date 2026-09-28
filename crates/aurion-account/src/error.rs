use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum AccountError {
    #[error("Perangkat {0:?} tidak terdaftar dalam akun ini")]
    DeviceNotRegistered(PublicKeyBytes),

    #[error("Perangkat {0:?} sudah kedaluwarsa")]
    DeviceExpired(PublicKeyBytes),

    #[error("Aksi ini membutuhkan hak akses Master Device")]
    MasterPrivilegeRequired,

    #[error("Jumlah transfer ({amount}) melebihi batas per transaksi ({limit})")]
    ExceedsPerTxLimit { amount: u64, limit: u64 },

    #[error("Pengeluaran kumulatif ({spent}) melebihi kuota harian ({limit})")]
    ExceedsDailyQuota { spent: u64, limit: u64 },

    #[error("Saldo tidak mencukupi: tersedia {available}, dibutuhkan {required}")]
    InsufficientBalance { available: u64, required: u64 },

    #[error("Nonce akun tidak cocok: diharapkan {expected}, diterima {got}")]
    InvalidNonce { expected: u64, got: u64 },

    #[error("Perangkat master tidak boleh dihapus")]
    CannotRemoveMasterDevice,

    #[error("Batas maksimum perangkat ({0}) telah tercapai")]
    DeviceLimitReached(usize),
}
