use crate::error::ConsensusError;
use aurion_criptografi::{
    verifikasi_tanda_tangan, Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier,
};

pub const PRECOMMIT_VOTE_SIZE: usize = 140;
pub const PRECOMMIT_PREIMAGE_SIZE: usize = 76;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

/// Signed timeout announcement for one consensus height and round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutVote {
    pub validator: PublicKeyBytes,
    pub height: u64,
    pub round: u32,
    pub signature: SignatureBytes,
}

impl TimeoutVote {
    #[must_use]
    pub const fn new(
        validator: PublicKeyBytes,
        height: u64,
        round: u32,
        signature: SignatureBytes,
    ) -> Self {
        Self {
            validator,
            height,
            round,
            signature,
        }
    }

    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_BFT_TIMEOUT_V1");
        hasher.update(&self.validator);
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.round.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Verify the cryptographic signature on this timeout announcement.
    ///
    /// # Errors
    /// Returns `InvalidVoteSignature` when the signer or signature is invalid.
    pub fn verify(&self) -> Result<(), ConsensusError> {
        SignatureVerifier::verify_single(&self.validator, &self.digest(), &self.signature)
            .map_err(|_| ConsensusError::InvalidVoteSignature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecommitVote {
    pub round: u32,
    pub height: u64,
    pub block_hash: Hash256,
    pub voter_pubkey: PublicKeyBytes,
    pub signature: SignatureBytes,
}

impl PrecommitVote {
    #[must_use]
    pub fn new(
        round: u32,
        height: u64,
        block_hash: Hash256,
        voter_pubkey: PublicKeyBytes,
        signature: SignatureBytes,
    ) -> Self {
        Self {
            round,
            height,
            block_hash,
            voter_pubkey,
            signature,
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; PRECOMMIT_VOTE_SIZE] {
        let mut buf = [0u8; PRECOMMIT_VOTE_SIZE];
        buf[0..4].copy_from_slice(&self.round.to_le_bytes());
        buf[4..12].copy_from_slice(&self.height.to_le_bytes());
        buf[12..44].copy_from_slice(&self.block_hash);
        buf[44..76].copy_from_slice(&self.voter_pubkey);
        buf[76..140].copy_from_slice(&self.signature);
        buf
    }

    /// Decode a 140-byte precommit vote buffer.
    ///
    /// # Errors
    /// Returns `ConsensusError` if buffer length is invalid.
    ///
    /// # Panics
    /// Panics if buffer length is not exactly 140 bytes (internal invariant).
    pub fn decode(buf: &[u8; PRECOMMIT_VOTE_SIZE]) -> Result<Self, ConsensusError> {
        let round = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let height = u64::from_le_bytes(buf[4..12].try_into().unwrap_or([0u8; 8]));
        let mut block_hash = [0u8; 32];
        block_hash.copy_from_slice(&buf[12..44]);
        let mut voter_pubkey = [0u8; 32];
        voter_pubkey.copy_from_slice(&buf[44..76]);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&buf[76..140]);
        Ok(Self::new(
            round,
            height,
            block_hash,
            voter_pubkey,
            signature,
        ))
    }

    /// Verify the cryptographic signature on this precommit vote.
    ///
    /// # Errors
    /// Returns `ConsensusError` if signature verification fails.
    pub fn verify(&self) -> Result<bool, ConsensusError> {
        let mut preimage = Vec::with_capacity(PRECOMMIT_PREIMAGE_SIZE);
        preimage.extend_from_slice(&self.round.to_le_bytes());
        preimage.extend_from_slice(&self.height.to_le_bytes());
        preimage.extend_from_slice(&self.block_hash);
        preimage.extend_from_slice(&self.voter_pubkey);
        Ok(verifikasi_tanda_tangan(&self.voter_pubkey, &preimage, &self.signature).is_ok())
    }
}
