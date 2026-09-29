use crate::{PrivateKeyBytes, PublicKeyBytes, SignatureBytes};
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;
use zeroize::Zeroize;

/// Struktur kunci privat dengan proteksi Zeroize (otomatis dihapus dari RAM saat drop).
pub struct Keypair {
    verifying_key: ed25519_dalek::VerifyingKey,
    signing_key: SigningKey,
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Keypair")
            .field("public_key", &self.verifying_key.to_bytes())
            .field("signing_key", &"[REDACTED]")
            .finish()
    }
}

impl Drop for Keypair {
    fn drop(&mut self) {
        // Defense-in-depth: wipe secret scalar on drop.
        // Inner SigningKey already wipes via its own ZeroizeOnDrop,
        // this ensures wrapper reuse-after-drop cannot leak.
        self.zeroize();
    }
}

impl Zeroize for Keypair {
    fn zeroize(&mut self) {
        // Secret material is wiped via SigningKey's Drop impl; there is no
        // public Zeroize impl on SigningKey in ed25519-dalek 2.x.
        // Rebuild keys from zero bytes so re-use after zeroize is safe.
        self.signing_key = SigningKey::from_bytes(&[0u8; 32]);
        self.verifying_key = self.signing_key.verifying_key();
    }
}

impl Keypair {
    /// Generate kunci baru dari random generator perangkat keras OS.
    #[must_use]
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            verifying_key,
            signing_key,
        }
    }

    /// Load kunci privat dari 32 bita raw.
    #[must_use]
    pub fn from_bytes(bytes: &PrivateKeyBytes) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        let verifying_key = signing_key.verifying_key();
        Self {
            verifying_key,
            signing_key,
        }
    }

    #[must_use]
    pub fn public_key_bytes(&self) -> PublicKeyBytes {
        self.verifying_key.to_bytes()
    }

    /// Ekspos kunci privat 32-byte (mis. untuk utilitas keygen CLI).
    ///
    /// Nilai biner ini adalah materi rahasia: jangan pernah mencatat
    /// atau mengirimkannya ke jaringan.
    #[must_use]
    pub fn private_key_bytes(&self) -> PrivateKeyBytes {
        self.signing_key.to_bytes()
    }

    /// Tanda tangani pesan/hash transaksi secara deterministik.
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> SignatureBytes {
        self.signing_key.sign(message).to_bytes()
    }
}
