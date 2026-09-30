use aurion_core::types::Quanta;
use aurion_execution::ExecutionError;
use thiserror::Error;

use crate::types::ChannelId;

/// Galat spesifik mesin state channel. Seluruh alur eksekusi keeper
/// mengembalikan `Result<T, ChannelError>` — dilarang panic/unwrap.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ChannelError {
    #[error("Saluran {0:?} tidak ditemukan")]
    ChannelNotFound(ChannelId),

    #[error("Saluran {0:?} sudah ada")]
    ChannelAlreadyExists(ChannelId),

    #[error("Tanda tangan BalanceProof tidak sah")]
    InvalidSignature,

    #[error("Nonce usang: diterima {provided}, terkonfirmasi {current}")]
    StaleNonce { provided: u64, current: u64 },

    #[error("Transfer melebihi escrow saluran: diminta {requested}, escrow {deposit}")]
    TransferExceedsDeposit { requested: Quanta, deposit: Quanta },

    #[error("Jendela sanggah masih aktif: sisa {remaining_blocks} blok")]
    ChallengePeriodActive { remaining_blocks: u64 },

    #[error("Overflow aritmetika u128")]
    ArithmeticOverflow,

    #[error("Serialisasi biner gagal: {0}")]
    SerializationFailure(String),

    #[error("Saldo sender tidak mencukupi: tersedia {available}, dibutuhkan {required}")]
    InsufficientSenderBalance { available: Quanta, required: Quanta },

    #[error("Saluran telah diselesaikan (Settled) dan terkunci permanen")]
    ChannelSettled,

    #[error("Operasi membutuhkan status Challenging, tetapi saluran tidak dalam jendela sanggah")]
    NotInChallengeWindow,
}

impl From<ExecutionError> for ChannelError {
    fn from(err: ExecutionError) -> Self {
        match err {
            ExecutionError::InsufficientBalance {
                available,
                required,
            } => Self::InsufficientSenderBalance {
                available,
                required,
            },
            ExecutionError::ArithmeticOverflow => Self::ArithmeticOverflow,
            other => Self::SerializationFailure(format!("Galat store/execution: {other}")),
        }
    }
}
