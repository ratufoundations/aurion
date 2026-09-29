use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("Magic bita jaringan tidak cocok: diharapkan {expected:#X}, diterima {got:#X}")]
    InvalidMagicBytes { expected: u32, got: u32 },

    #[error("Ukuran frame melebihi batas aman ({size} bita > batas {max_allowed} bita)")]
    FrameTooLarge { size: usize, max_allowed: usize },

    #[error("ID tipe pesan tidak dikenal: {0}")]
    UnknownMessageType(u8),

    #[error("Payload pesan korup atau panjang bita tidak valid")]
    MalformedPayload,

    #[error("Aliran byte terpotong: frame tidak lengkap saat koneksi berakhir")]
    UnexpectedEof,

    #[error("Chain ID peer tidak cocok: diharapkan {expected}, diterima {got}")]
    ChainIdMismatch { expected: u64, got: u64 },

    #[error("Versi protokol tidak kompatibel: diharapkan {expected}, diterima {got}")]
    IncompatibleProtocolVersion { expected: u16, got: u16 },

    #[error("Pesan sebelum handshake: lalu lintas tanpa autentikasi ditolak")]
    UnauthenticatedMessage,

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
