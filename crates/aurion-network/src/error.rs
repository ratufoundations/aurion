use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("Magic bita jaringan tidak cocok: diharapkan {expected:#X}, diterima {got:#X}")]
    InvalidMagic { expected: u32, got: u32 },

    #[error("Ukuran frame melebihi batas aman ({size} bita > batas {limit} bita)")]
    FrameTooLarge { size: usize, limit: usize },

    #[error("ID tipe pesan tidak dikenal: {0}")]
    UnknownMessageType(u8),

    #[error("Payload pesan korup atau panjang bita tidak valid")]
    MalformedPayload,

    #[error("Kesalahan deserialisasi ledger: {0}")]
    LedgerCodec(String),

    #[error("Koneksi peer terputus")]
    ConnectionClosed,
}

impl From<std::io::Error> for NetworkError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<aurion_ledger::LedgerError> for NetworkError {
    fn from(e: aurion_ledger::LedgerError) -> Self {
        Self::LedgerCodec(e.to_string())
    }
}
