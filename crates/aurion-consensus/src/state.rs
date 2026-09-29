use crate::{
    error::ConsensusError,
    validator::ValidatorSet,
    vote::{TimeoutVote, Vote, VoteType},
};
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumCertificate {
    pub height: u64,
    pub round: u32,
    pub block_hash: Hash256,
    pub vote_type: VoteType,
    pub signers: Vec<PublicKeyBytes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivocationEvidence {
    pub first: Vote,
    pub second: Vote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutCertificate {
    pub height: u64,
    pub timed_out_round: u32,
    pub next_round: u32,
    pub signers: Vec<PublicKeyBytes>,
    pub next_leader: PublicKeyBytes,
}

#[derive(Debug)]
pub struct RoundState {
    pub height: u64,
    pub round: u32,
    pub validator_set: ValidatorSet,
    prevotes: BTreeMap<Hash256, BTreeSet<PublicKeyBytes>>,
    precommits: BTreeMap<Hash256, BTreeSet<PublicKeyBytes>>,
    voted: BTreeMap<(VoteType, PublicKeyBytes), Vote>,
    equivocations: Vec<EquivocationEvidence>,
    timeout_votes: BTreeSet<PublicKeyBytes>,
    locked_qc: Option<QuorumCertificate>,
}

impl RoundState {
    pub fn new(height: u64, round: u32, validator_set: ValidatorSet) -> Self {
        tracing::info!(height, round, "Ronde BFT baru dimulai");
        Self {
            height,
            round,
            validator_set,
            prevotes: BTreeMap::new(),
            precommits: BTreeMap::new(),
            voted: BTreeMap::new(),
            equivocations: Vec::new(),
            timeout_votes: BTreeSet::new(),
            locked_qc: None,
        }
    }

    /// Validate and accumulate a vote, producing a QC when the supermajority is reached.
    ///
    /// # Errors
    /// Rejects stale/mismatched votes, unknown validators, invalid signatures, duplicates,
    /// and equivocation without adding the rejected vote to a quorum accumulator.
    pub fn add_vote(&mut self, vote: &Vote) -> Result<Option<QuorumCertificate>, ConsensusError> {
        if vote.height < self.height || (vote.height == self.height && vote.round < self.round) {
            return Err(ConsensusError::StaleVote);
        }
        if vote.height != self.height || vote.round != self.round {
            return Err(ConsensusError::HeightRoundMismatch);
        }
        if !self.validator_set.is_validator(&vote.validator) {
            return Err(ConsensusError::UnknownValidator(vote.validator));
        }
        vote.verify()?;

        let vote_key = (vote.vote_type, vote.validator);
        if let Some(first) = self.voted.get(&vote_key) {
            if first.block_hash != vote.block_hash {
                self.equivocations.push(EquivocationEvidence {
                    first: first.clone(),
                    second: vote.clone(),
                });
                return Err(ConsensusError::EquivocationDetected(vote.validator));
            }
            return Err(ConsensusError::DuplicateVote(vote.validator));
        }

        let threshold = self.validator_set.quorum_threshold()?;
        let vote_map = match vote.vote_type {
            VoteType::Prevote => &mut self.prevotes,
            VoteType::Precommit => &mut self.precommits,
        };
        let voters = vote_map.entry(vote.block_hash).or_default();
        if !voters.insert(vote.validator) {
            return Err(ConsensusError::DuplicateVote(vote.validator));
        }
        self.voted.insert(vote_key, vote.clone());

        tracing::debug!(voter = ?vote.validator, height = vote.height, round = vote.round, "Suara vote sah diterima");
        if voters.len() >= threshold {
            tracing::info!(
                height = self.height,
                round = self.round,
                signers = voters.len(),
                "Kuorum QC 2f+1 tercapai"
            );
            let qc = QuorumCertificate {
                height: self.height,
                round: self.round,
                block_hash: vote.block_hash,
                vote_type: vote.vote_type,
                signers: voters.iter().copied().collect(),
            };
            if vote.vote_type == VoteType::Prevote {
                self.locked_qc = Some(qc.clone());
            }
            return Ok(Some(qc));
        }
        Ok(None)
    }

    #[must_use]
    pub fn vote_count(&self, vote_type: VoteType, block_hash: &Hash256) -> usize {
        let vote_map = match vote_type {
            VoteType::Prevote => &self.prevotes,
            VoteType::Precommit => &self.precommits,
        };
        vote_map.get(block_hash).map_or(0, BTreeSet::len)
    }

    #[must_use]
    pub fn locked_qc(&self) -> Option<&QuorumCertificate> {
        self.locked_qc.as_ref()
    }

    #[must_use]
    pub fn equivocation_evidence(&self) -> &[EquivocationEvidence] {
        &self.equivocations
    }

    #[must_use]
    pub fn timeout_vote_count(&self) -> usize {
        self.timeout_votes.len()
    }

    /// Advance exactly one round and reset its vote accumulators.
    ///
    /// # Errors
    /// Returns an error if the round counter cannot be incremented.
    pub fn advance_round(&mut self) -> Result<(), ConsensusError> {
        self.round = self
            .round
            .checked_add(1)
            .ok_or(ConsensusError::ArithmeticOverflow)?;
        self.prevotes.clear();
        self.precommits.clear();
        self.voted.clear();
        self.timeout_votes.clear();
        tracing::info!(
            height = self.height,
            round = self.round,
            "Round timeout; pacemaker maju ke ronde berikutnya"
        );
        Ok(())
    }

    /// Validate signed timeout messages and move to the next round on a timeout quorum.
    ///
    /// # Errors
    /// Rejects stale/mismatched messages, unknown validators, invalid signatures, duplicates,
    /// and checked-arithmetic failures.
    pub fn add_timeout_vote(
        &mut self,
        vote: &TimeoutVote,
    ) -> Result<Option<TimeoutCertificate>, ConsensusError> {
        if vote.height < self.height || (vote.height == self.height && vote.round < self.round) {
            return Err(ConsensusError::StaleVote);
        }
        if vote.height != self.height || vote.round != self.round {
            return Err(ConsensusError::HeightRoundMismatch);
        }
        if !self.validator_set.is_validator(&vote.validator) {
            return Err(ConsensusError::UnknownValidator(vote.validator));
        }
        vote.verify()?;
        let threshold = self.validator_set.quorum_threshold()?;
        if self.timeout_votes.contains(&vote.validator) {
            return Err(ConsensusError::DuplicateTimeoutVote(vote.validator));
        }
        let projected_count = self
            .timeout_votes
            .len()
            .checked_add(1)
            .ok_or(ConsensusError::ArithmeticOverflow)?;
        let timed_out_round = self.round;
        let next_round = if projected_count >= threshold {
            Some(
                timed_out_round
                    .checked_add(1)
                    .ok_or(ConsensusError::ArithmeticOverflow)?,
            )
        } else {
            None
        };
        self.timeout_votes.insert(vote.validator);
        let Some(next_round) = next_round else {
            return Ok(None);
        };
        let certificate = TimeoutCertificate {
            height: self.height,
            timed_out_round,
            next_round,
            signers: self.timeout_votes.iter().copied().collect(),
            next_leader: self.validator_set.leader_for(self.height, next_round)?,
        };
        self.advance_round()?;
        Ok(Some(certificate))
    }
}
