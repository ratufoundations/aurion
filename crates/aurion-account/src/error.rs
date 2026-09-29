use crate::role::Role;
use aurion_criptografi::{Hash256, PublicKeyBytes};
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

    #[error("Alamat akun harus 32 bita, diterima {got} bita")]
    InvalidAccountIdLength { got: usize },

    #[error("Peran tidak berwenang untuk aksi ini: dibutuhkan {expected:?}, dimiliki {actual:?}")]
    UnauthorizedRole { expected: Role, actual: Role },

    #[error(
        "Ambang batas multi-sig tidak valid: {approvals} persetujuan sah, ambang {threshold}, penandatangan {signers}"
    )]
    InvalidThreshold {
        approvals: usize,
        threshold: usize,
        signers: usize,
    },

    #[error("Chain ID tidak cocok: diharapkan {expected}, diterima {got}")]
    InvalidChainId { expected: u64, got: u64 },

    #[error("Penandatangan tidak cocok dengan otoritas yang diharapkan")]
    SignerMismatch,

    #[error("Digest aksi tidak cocok dengan otorisasi delegasi yang ditandatangani")]
    ActionDigestMismatch,

    #[error(
        "Otorisasi delegasi pada nonce {nonce} untuk akun {account:?} sudah pernah dieksekusi"
    )]
    ReplayDetected { account: Hash256, nonce: u64 },

    #[error("Tanda tangan otorisasi tidak valid untuk aksi ini")]
    InvalidSignature,

    #[error("Jumlah {amount} di bawah ambang dust akun baru ({minimum})")]
    BelowDustThreshold { amount: u64, minimum: u64 },

    #[error("Stake tidak mencukupi: tersedia {provided}, dibutuhkan {required}")]
    InsufficientStake { provided: u64, required: u64 },

    #[error("Aritmetika Quanta meluap (overflow/underflow)")]
    ArithmeticOverflow,

    #[error("Mutasi kebijakan akan mengunci akun secara permanen (bricked state)")]
    BrickedStateMutation,
}
