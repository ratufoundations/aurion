use crate::error::ValidatorError;
use aurion_account::AccountId;
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes, SignatureBytes, SignatureVerifier};

/// Tag pemisahan domain kanonikal bukti kepemilikan kunci konsensus.
pub const POP_DOMAIN_TAG: &[u8] = b"AURION_VALIDATOR_POP_V1";

/// Bukti kepemilikan kunci konsensus (*Proof-of-Possession*).
///
/// Kandidat membuktikan penguasaan kunci privat konsensus `Ed25519` dengan
/// menandatangani komitmen `DOMAIN_TAG || account_id || consensus_pubkey`.
/// Karena `account_id` ikut ditandatangani, bukti tidak dapat dipindahkan ke
/// akun berdaulat lain maupun dipakai ulang untuk kunci konsensus lain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOfPossession {
    /// Akun berdaulat pemilik kunci konsensus.
    pub account: AccountId,
    /// Kunci publik konsensus yang dibuktikan kepemilikannya.
    pub consensus_pubkey: PublicKeyBytes,
    /// Tanda tangan `Ed25519` atas [`ProofOfPossession::digest`].
    pub signature: SignatureBytes,
}

impl ProofOfPossession {
    /// Komitmen biner yang ditandatangani kandidat.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(POP_DOMAIN_TAG);
        hasher.update(&self.account);
        hasher.update(&self.consensus_pubkey);
        *hasher.finalize().as_bytes()
    }

    /// Terbitkan bukti kepemilikan dengan menandatangani komitmen kanonikal.
    #[must_use]
    pub fn issue(account: AccountId, consensus_key: &Keypair) -> Self {
        let consensus_pubkey = consensus_key.public_key_bytes();
        let mut proof = Self {
            account,
            consensus_pubkey,
            signature: [0_u8; 64],
        };
        proof.signature = consensus_key.sign(&proof.digest());
        proof
    }

    /// Verifikasi binding akun dan keaslian tanda tangan kunci konsensus.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::InvalidProofOfPossession` bila bukti tidak
    /// terikat pada akun yang diharapkan atau tanda tangannya tidak sah.
    pub fn verify(&self, expected_account: &AccountId) -> Result<(), ValidatorError> {
        if self.account != *expected_account {
            return Err(ValidatorError::InvalidProofOfPossession);
        }
        SignatureVerifier::verify_single(&self.consensus_pubkey, &self.digest(), &self.signature)
            .map_err(|_| ValidatorError::InvalidProofOfPossession)
    }
}
