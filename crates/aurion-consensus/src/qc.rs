use crate::error::ConsensusError;
use crate::validator::ValidatorSet;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes};

/// Calculate adaptive quorum threshold using integer-only arithmetic.
///
/// Formula: `Q(N) = floor(2N/3) + 1`
///
/// # Errors
/// Returns `ConsensusError::EmptyValidatorSet` if `n == 0`.
pub const fn calculate_quorum(n: usize) -> Result<usize, ConsensusError> {
    if n == 0 {
        return Err(ConsensusError::EmptyValidatorSet);
    }
    Ok((2 * n) / 3 + 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumCertificate {
    pub block_hash: Hash256,
    pub height: u64,
    pub round: u32,
    pub votes: Vec<(PublicKeyBytes, SignatureBytes)>,
}

impl QuorumCertificate {
    #[must_use]
    pub fn new(
        block_hash: Hash256,
        height: u64,
        round: u32,
        votes: Vec<(PublicKeyBytes, SignatureBytes)>,
    ) -> Self {
        Self {
            block_hash,
            height,
            round,
            votes,
        }
    }

    /// Verify quorum certificate against validator set.
    ///
    /// # Errors
    /// Returns `ConsensusError` if quorum not reached, duplicate votes, or invalid signatures.
    pub fn verify(&self, validator_set: &ValidatorSet) -> Result<bool, ConsensusError> {
        let threshold = calculate_quorum(validator_set.total_validators())?;
        if self.votes.len() < threshold {
            return Err(ConsensusError::QuorumNotReached {
                collected: self.votes.len(),
                required: threshold,
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for (pubkey, signature) in &self.votes {
            if !validator_set.is_validator(pubkey) {
                return Err(ConsensusError::UnknownValidator(*pubkey));
            }
            if !seen.insert(*pubkey) {
                return Err(ConsensusError::DuplicateVote(*pubkey));
            }
            let mut preimage = Vec::with_capacity(76);
            preimage.extend_from_slice(&self.round.to_le_bytes());
            preimage.extend_from_slice(&self.height.to_le_bytes());
            preimage.extend_from_slice(&self.block_hash);
            preimage.extend_from_slice(pubkey);
            aurion_criptografi::verifikasi_tanda_tangan(pubkey, &preimage, signature)
                .map_err(|_| ConsensusError::InvalidVoteSignature)?;
        }
        Ok(true)
    }
}
