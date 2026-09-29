use aurion_criptografi::PublicKeyBytes;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSet {
    pub members: BTreeSet<PublicKeyBytes>,
}

impl ValidatorSet {
    #[must_use]
    pub fn new(validators: Vec<PublicKeyBytes>) -> Self {
        Self {
            members: validators.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn total_validators(&self) -> usize {
        self.members.len()
    }

    #[must_use]
    pub fn is_validator(&self, pubkey: &PublicKeyBytes) -> bool {
        self.members.contains(pubkey)
    }

    /// Menghitung toleransi Byzantine f = (N - 1) / 3
    #[must_use]
    pub fn max_faulty_nodes(&self) -> usize {
        let n = self.total_validators();
        if n == 0 {
            0
        } else {
            (n - 1) / 3
        }
    }

    /// Hitung kuorum BFT `2f + 1` dengan checked arithmetic.
    ///
    /// # Errors
    /// Mengembalikan `ArithmeticOverflow` jika ambang tidak dapat direpresentasikan.
    pub fn quorum_threshold(&self) -> Result<usize, crate::ConsensusError> {
        self.max_faulty_nodes()
            .checked_mul(2)
            .and_then(|faulty| faulty.checked_add(1))
            .ok_or(crate::ConsensusError::ArithmeticOverflow)
    }

    /// Select the deterministic round-robin leader at a given height and round.
    ///
    /// # Errors
    /// Returns an error for an empty validator set or when the index arithmetic overflows.
    pub fn leader_for(
        &self,
        height: u64,
        round: u32,
    ) -> Result<PublicKeyBytes, crate::ConsensusError> {
        let member_count = u64::try_from(self.members.len())
            .map_err(|_| crate::ConsensusError::ArithmeticOverflow)?;
        if member_count == 0 {
            return Err(crate::ConsensusError::EmptyValidatorSet);
        }
        let round = u64::from(round);
        let index = height
            .checked_add(round)
            .ok_or(crate::ConsensusError::ArithmeticOverflow)?
            % member_count;
        let index =
            usize::try_from(index).map_err(|_| crate::ConsensusError::ArithmeticOverflow)?;
        self.members
            .iter()
            .nth(index)
            .copied()
            .ok_or(crate::ConsensusError::EmptyValidatorSet)
    }
}
