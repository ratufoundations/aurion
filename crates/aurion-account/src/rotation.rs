use crate::{address::AccountId, error::AccountError};
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

/// Bukti rotasi kunci master akun yang ditandatangani kunci master aktif.
///
/// Bukti mengikat alamat akun, kunci lama, kunci baru, dan nonce akun sehingga
/// tidak dapat dipakai ulang pada akun lain atau diputar kembali (replay).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRotationProof {
    pub account: AccountId,
    pub old_key: PublicKeyBytes,
    pub new_key: PublicKeyBytes,
    pub nonce: u64,
    pub signature: SignatureBytes,
}

impl KeyRotationProof {
    /// Tag pemisahan domain kanonikal untuk bukti rotasi kunci.
    pub const DOMAIN_TAG: &'static [u8] = b"AURION_KEY_ROTATION_V1";

    /// Komitmen biner bukti: `DOMAIN_TAG || account || old || new || nonce`.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(Self::DOMAIN_TAG);
        hasher.update(&self.account);
        hasher.update(&self.old_key);
        hasher.update(&self.new_key);
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Verifikasi binding akun/nonce dan tanda tangan kunci master lama.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::SignerMismatch` bila bukti tidak terikat pada
    /// akun atau kunci yang sama, `AccountError::InvalidNonce` bila nonce tidak
    /// cocok, dan `AccountError::InvalidSignature` bila tanda tangan kunci lama
    /// tidak sah.
    pub fn verify(&self, account: &AccountId, nonce: u64) -> Result<(), AccountError> {
        if &self.account != account {
            return Err(AccountError::SignerMismatch);
        }
        if self.old_key == self.new_key {
            return Err(AccountError::SignerMismatch);
        }
        if self.nonce != nonce {
            return Err(AccountError::InvalidNonce {
                expected: nonce,
                got: self.nonce,
            });
        }
        SignatureVerifier::verify_single(&self.old_key, &self.digest(), &self.signature)
            .map_err(|_| AccountError::InvalidSignature)
    }
}
