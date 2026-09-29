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

    /// Kuorum BFT standar: 2f + 1
    #[must_use]
    pub fn quorum_threshold(&self) -> usize {
        2 * self.max_faulty_nodes() + 1
    }
}
