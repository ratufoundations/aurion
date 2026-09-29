use thiserror::Error;

/// Tipe galat terstruktur untuk subsistem Gateway (boundary ingress & CQRS).
#[derive(Error, Debug)]
pub enum GatewayError {
    // ========================================================================
    // [GW0] Sanitasi Ingress & Penolakan Data Cacat
    // ========================================================================

    #[error("Payload kosong: tidak ada data yang diterima")]
    EmptyPayload,

    #[error("Payload terlalu besar: ukuran {size} byte melebihi batas {max_allowed} byte")]
    PayloadTooLarge { size: u64, max_allowed: u64 },

    #[error("Format payload tidak valid: {0}")]
    MalformedPayload(String),

    #[error("Encoding tidak valid: {0}")]
    InvalidEncoding(String),

    #[error("Nilai negatif tidak didukung untuk field {field}: {value}")]
    NegativeValue { field: String, value: i64 },

    #[error("Tipe float tidak didukung untuk field {field}: {value}")]
    FloatValue { field: String, value: String },

    #[error("Format heksadesimal tidak valid: panjang ganjil {length}")]
    InvalidHexFormat { length: usize },

    #[error("Karakter Unicode ilegal terdeteksi: {0}")]
    InvalidUnicode(String),

    // ========================================================================
    // [GW1] Pemisahan Tegas Jalur CQRS
    // ========================================================================

    #[error("Endpoint write-only dipanggil melalui jalur read: {endpoint}")]
    WriteEndpointOnReadPath { endpoint: String },

    #[error("Mutasi state melalui endpoint kueri ditolak: {action}")]
    StateMutationOnReadPath { action: String },

    #[error("Mempool penuh: {current}/{capacity} transaksi")]
    MempoolFull { current: usize, capacity: usize },

    // ========================================================================
    // [GW2] Pertahanan Batas Ukuran & Anti-DoS
    // ========================================================================

    #[error("Ukuran frame melebihi batas aman: {size} > {max}")]
    FrameTooLarge { size: u64, max: u64 },

    #[error("Alokasi buffer gagal: ukuran {size} terlalu besar")]
    BufferAllocationFailed { size: u64 },

    // ========================================================================
    // [GW3] Pembatasan Laju Nir-Pecahan (Token Bucket)
    // ========================================================================

    #[error("Rate limit terlampaui: silakan coba lagi setelah {retry_after_ms} ms")]
    RateLimitExceeded { retry_after_ms: u64 },

    #[error("Kuota token bucket habis untuk identitas {identity}")]
    TokenBucketExhausted { identity: String },

    // ========================================================================
    // [GW4] Isolasi Kegagalan Klien & Penutupan Galat Internal
    // ========================================================================

    #[error("Galat internal: {trace_id}")]
    InternalError { trace_id: String },

    #[error("Koneksi klien terputus: {reason}")]
    ClientDisconnected { reason: String },

    #[error("Timeout permintaan: {timeout_ms} ms")]
    RequestTimeout { timeout_ms: u64 },

    // ========================================================================
    // [GW5] Metrik Operasional & Kuota Ingress Nir-Pecahan
    // ========================================================================

    #[error("Akumulasi metrik overflow: {field}")]
    MetricsOverflow { field: String },

    // ========================================================================
    // Sistem & Integrasi
    // ========================================================================

    #[error("Kesalahan Zenoh: {0}")]
    Zenoh(#[from] zenoh::Error),

    #[error("Payload transaksi korup atau tidak valid: {0}")]
    InvalidTransactionPayload(String),

    #[error("Alamat key expression tidak dikenal: {0}")]
    UnknownKeyExpression(String),

    #[error("Kesalahan pembacaan ledger: {0}")]
    Ledger(#[from] aurion_ledger::LedgerError),

    #[error("Penolakan dari antrean mempool: {0}")]
    MempoolRejected(#[from] aurion_mempool::MempoolError),
}

impl GatewayError {
    /// Dapatkan trace ID untuk galat internal (GW4).
    #[must_use]
    pub fn trace_id(&self) -> Option<&str> {
        match self {
            Self::InternalError { trace_id } => Some(trace_id),
            _ => None,
        }
    }

    /// Periksa apakah galat ini berhubungan dengan rate limiting (GW3).
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        matches!(self, Self::RateLimitExceeded { .. } | Self::TokenBucketExhausted { .. })
    }

    /// Periksa apakah galat ini berhubungan dengan ukuran payload (GW2).
    #[must_use]
    pub fn is_size_related(&self) -> bool {
        matches!(
            self,
            Self::PayloadTooLarge { .. } | Self::FrameTooLarge { .. } | Self::BufferAllocationFailed { .. }
        )
    }

    /// Periksa apakah galat ini berhubungan dengan sanitasi input (GW0).
    #[must_use]
    pub fn is_sanitization_error(&self) -> bool {
        matches!(
            self,
            Self::EmptyPayload
                | Self::MalformedPayload(_)
                | Self::InvalidEncoding(_)
                | Self::NegativeValue { .. }
                | Self::FloatValue { .. }
                | Self::InvalidHexFormat { .. }
                | Self::InvalidUnicode(_)
        )
    }

    /// Dapatkan waktu retry untuk galat rate limit (GW3).
    #[must_use]
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Self::RateLimitExceeded { retry_after_ms } => Some(*retry_after_ms),
            _ => None,
        }
    }
}
