use aurion_core::types::Quanta;
use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum MempoolError {
    #[error("Tanda tangan kriptografi transaksi tidak sah")]
    InvalidSignature,

    #[error("Nonce terlalu rendah: status terkonfirmasi {expected}, diterima {got}")]
    NonceTooLow { expected: u64, got: u64 },

    #[error("Transaksi dengan nonce {nonce} dari akun {sender:?} sudah ada di mempool")]
    DuplicateNonce { sender: PublicKeyBytes, nonce: u64 },

    #[error("Transaksi identik sudah ada di mempool")]
    DuplicateTransaction,

    #[error("Saldo tidak mencukupi: tersedia {available}, dibutuhkan total {required}")]
    InsufficientBalance { available: Quanta, required: Quanta },

    #[error("Overflow saat menghitung kebutuhan saldo transaksi")]
    ArithmeticOverflow,

    #[error("Fee {fee} di bawah minimum jaringan {minimum}")]
    FeeTooLow { fee: Quanta, minimum: Quanta },

    #[error("Kapasitas maksimum mempool ({capacity}) tercapai")]
    PoolCapacityReached { capacity: usize },

    #[error("Mempool penuh; fee transaksi tidak cukup untuk menggusur transaksi terendah")]
    PoolFull,

    #[error("Akun {0:?} melebihi batas antrean transaksi ({1})")]
    AccountQueueLimitExceeded(PublicKeyBytes, usize),

    #[error("Transfer ke diri sendiri dilarang")]
    SelfTransfer,

    #[error("Jumlah transfer harus lebih dari 0")]
    ZeroAmount,
}
