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
