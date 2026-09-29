#![forbid(unsafe_code)]

pub mod hash;
pub mod signature;

pub use hash::Hasher;
pub use signature::{CryptoError, Keypair, SignatureVerifier};

// Tipe data baku berukuran tetap (fixed-size byte arrays)
pub type Hash256 = [u8; 32];
pub type PublicKeyBytes = [u8; 32];
pub type SignatureBytes = [u8; 64];
pub type PrivateKeyBytes = [u8; 32];

/// Verifikasi tanda tangan Ed25519 untuk kompatibilitas dengan modul eksekusi.
/// 
/// # Arguments
/// * `public_key` - Kunci publik 32-byte dari pengirim
/// * `message` - Pesan/preimage yang ditandatangani
/// * `signature` - Tanda tangan digital 64-byte
/// 
/// # Errors
/// Mengembalikan `CryptoError` jika verifikasi gagal
pub fn verifikasi_tanda_tangan(
    public_key: &PublicKeyBytes,
    message: &[u8],
    signature: &SignatureBytes,
) -> Result<(), CryptoError> {
    SignatureVerifier::verify_single(public_key, message, signature)
}
