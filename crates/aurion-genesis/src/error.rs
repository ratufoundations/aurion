use thiserror::Error;

#[derive(Error, Debug)]
pub enum GenesisError {
    #[error("Ledger sudah terinisialisasi pada ketinggian {0}, tidak dapat menimpa Genesis")]
    AlreadyInitialized(u64),

    #[error("Himpunan validator perdana tidak boleh kosong")]
    EmptyValidatorSet,

    #[error("Himpunan guard perdana tidak boleh kosong")]
    EmptyGuardSet,

    #[error("Terjadi luapan aritmatika saat menghitung total Quanta")]
    ArithmeticOverflow,

    #[error("Kesalahan penulisan ledger: {0}")]
    LedgerError(String),

    #[error("Kesalahan serialisasi JSON: {0}")]
    SerializationError(String),
}

impl From<aurion_ledger::LedgerError> for GenesisError {
    fn from(e: aurion_ledger::LedgerError) -> Self {
        Self::LedgerError(e.to_string())
    }
}

impl From<serde_json::Error> for GenesisError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}
