use crate::{
    error::ConsensusError,
    validator::ValidatorSet,
    vote::{Vote, VoteType},
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

#[derive(Debug)]
pub struct RoundState {
    pub height: u64,
    pub round: u32,
    pub validator_set: ValidatorSet,
    // block_hash -> Set Validator yang sudah vote
    prevotes: BTreeMap<Hash256, BTreeSet<PublicKeyBytes>>,
    precommits: BTreeMap<Hash256, BTreeSet<PublicKeyBytes>>,
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
        }
    }

    /// Masukkan vote ke pool, validasi validator & tanda tangan.
    ///
    /// # Errors
    /// Mengembalikan error bila ronde tidak cocok, validator tidak dikenal, vote duplikat,
    /// atau tanda tangan tidak valid.
    pub fn add_vote(&mut self, vote: &Vote) -> Result<Option<QuorumCertificate>, ConsensusError> {
        if vote.height != self.height || vote.round != self.round {
            return Err(ConsensusError::HeightRoundMismatch);
        }

        if !self.validator_set.is_validator(&vote.validator) {
            return Err(ConsensusError::UnknownValidator(vote.validator));
        }

        // 1. Verifikasi tanda tangan kriptografi
        vote.verify()?;

        // 2. Masukkan ke pool vote yang sesuai
        let vote_map = match vote.vote_type {
            VoteType::Prevote => &mut self.prevotes,
            VoteType::Precommit => &mut self.precommits,
        };

        let voters = vote_map.entry(vote.block_hash).or_default();
        if !voters.insert(vote.validator) {
            return Err(ConsensusError::DuplicateVote(vote.validator));
        }

        tracing::debug!(voter = ?vote.validator, height = vote.height, round = vote.round, "Suara vote sah diterima");

        // 3. Cek apakah batas kuorum 2f + 1 sudah terpenuhi
        let threshold = self.validator_set.quorum_threshold();
        if voters.len() >= threshold {
            tracing::info!(
                height = self.height,
                round = self.round,
                signers = voters.len(),
                "Kuorum QC 2f+1 tercapai"
            );
            return Ok(Some(QuorumCertificate {
                height: self.height,
                round: self.round,
                block_hash: vote.block_hash,
                vote_type: vote.vote_type,
                signers: voters.iter().copied().collect(),
            }));
        }

        Ok(None)
    }
}
