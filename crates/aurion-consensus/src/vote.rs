use crate::error::ConsensusError;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vote {
    pub validator: PublicKeyBytes,
    pub block_hash: Hash256,
    pub height: u64,
    pub round: u32,
    pub vote_type: VoteType,
    pub signature: SignatureBytes,
}

impl Vote {
    #[must_use]
    pub fn new(
        validator: PublicKeyBytes,
        block_hash: Hash256,
        height: u64,
        round: u32,
        vote_type: VoteType,
        signature: SignatureBytes,
    ) -> Self {
        Self {
            validator,
            block_hash,
            height,
            round,
            vote_type,
            signature,
        }
    }

    /// Serialisasi kanonikal data vote sebelum di-hash dan diverifikasi
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_BFT_VOTE_V1");
        hasher.update(&self.validator);
        hasher.update(&self.block_hash);
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.round.to_le_bytes());
        let type_byte = match self.vote_type {
            VoteType::Prevote => 0x01,
            VoteType::Precommit => 0x02,
        };
        hasher.update(&[type_byte]);
        *hasher.finalize().as_bytes()
    }

    /// Verifikasi kriptografis suara validator.
    ///
    /// # Errors
    /// Mengembalikan error bila tanda tangan suara tidak valid.
    pub fn verify(&self) -> Result<(), ConsensusError> {
        let digest = self.digest();
        SignatureVerifier::verify_single(&self.validator, &digest, &self.signature)
            .map_err(|_| ConsensusError::InvalidVoteSignature)
    }
}
