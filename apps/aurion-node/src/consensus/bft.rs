use aurion_consensus::{calculate_quorum, QuorumCertificate, ValidatorSet};
use aurion_criptografi::{PublicKeyBytes, SignatureBytes};

#[derive(Debug)]
pub struct BftConsensus {
    validator_set: ValidatorSet,
    round: u32,
    height: u64,
    votes: Vec<(PublicKeyBytes, SignatureBytes)>,
}

impl BftConsensus {
    #[must_use]
    pub fn new(validator_set: ValidatorSet, height: u64) -> Self {
        Self {
            validator_set,
            round: 0,
            height,
            votes: Vec::new(),
        }
    }

    pub fn add_vote(
        &mut self,
        pubkey: PublicKeyBytes,
        signature: SignatureBytes,
        block_hash: [u8; 32],
    ) -> Result<Option<QuorumCertificate>, Box<dyn std::error::Error>> {
        if !self.validator_set.is_validator(&pubkey) {
            return Err("unknown validator".into());
        }
        if self.votes.iter().any(|(p, _)| *p == pubkey) {
            return Err("duplicate vote".into());
        }
        self.votes.push((pubkey, signature));

        let threshold = calculate_quorum(self.validator_set.total_validators())?;
        if self.votes.len() >= threshold {
            let qc =
                QuorumCertificate::new(block_hash, self.height, self.round, self.votes.clone());
            return Ok(Some(qc));
        }
        Ok(None)
    }

    #[must_use]
    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }

    pub fn quorum_threshold(&self) -> Result<usize, Box<dyn std::error::Error>> {
        Ok(calculate_quorum(self.validator_set.total_validators())?)
    }
}
