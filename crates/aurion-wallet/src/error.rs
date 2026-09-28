use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum WalletError {
    #[error("Sertifikat delegasi perangkat tidak valid: tanda tangan tidak cocok")]
    InvalidDelegationSignature,

    #[error("Sertifikat delegasi perangkat telah kedaluwarsa: kedaluwarsa {expired_at}, waktu saat ini {current_time}")]
    DelegationExpired { expired_at: u64, current_time: u64 },

    #[error("Domain sertifikat tidak cocok dengan domain wallet")]
    DomainMismatch,

    #[error("Perangkat {0:?} tidak memiliki izin otorisasi untuk aksi ini")]
    UnauthorizedDevice(PublicKeyBytes),

    #[error("Identitas pemilik tidak cocok")]
    IdentityMismatch,

    #[error("Kesalahan kriptografi: {0}")]
    CryptoError(String),
}
