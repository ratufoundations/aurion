use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Format kunci publik tidak valid")]
    InvalidPublicKey,
    #[error("Format tanda tangan digital tidak valid")]
    InvalidSignature,
    #[error("Verifikasi tanda tangan gagal (palsu atau data korup)")]
    VerificationFailed,
    #[error("Ukuran batch tidak konsisten")]
    BatchMismatch,
}
