#![forbid(unsafe_code)]

use crate::{council::MINIMUM_GUARD_QUORUM, error::GuardError};
use aurion_core::types::{Quanta, QUANTA_PER_AUR};
use aurion_criptografi::PublicKeyBytes;

/// 660.000 AUR dalam satuan Quanta (1 AUR = 10.000.000.000 Quanta).
pub const DEFAULT_MIN_GUARD_STAKE_QUANTA: Quanta = 660_000 * QUANTA_PER_AUR;

/// Bobot penilaian kontribusi validator (Integer murni)
pub const WEIGHT_PROPOSED_BLOCK: u64 = 100;
pub const WEIGHT_VOTE_CAST: u64 = 10;
pub const WEIGHT_ACTIVE_EPOCH: u64 = 1_000;
pub const PENALTY_MISSED_ROUND: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateProfile {
    pub validator: PublicKeyBytes,
    pub stake_quanta: Quanta,
    pub blocks_proposed: u64,
    pub votes_cast: u64,
    pub missed_rounds: u64,
    pub active_epochs: u32,
    pub is_blacklisted: bool,
}

impl CandidateProfile {
    /// Hitung skor kontribusi validator secara deterministik tanpa float
    #[must_use]
    pub fn compute_score(&self) -> u64 {
        let proposed_score = self.blocks_proposed.saturating_mul(WEIGHT_PROPOSED_BLOCK);
        let vote_score = self.votes_cast.saturating_mul(WEIGHT_VOTE_CAST);
        let seniority_score = u64::from(self.active_epochs).saturating_mul(WEIGHT_ACTIVE_EPOCH);

        let positive_score = proposed_score
            .saturating_add(vote_score)
            .saturating_add(seniority_score);

        let penalty = self.missed_rounds.saturating_mul(PENALTY_MISSED_ROUND);

        positive_score.saturating_sub(penalty)
    }
}

#[derive(Debug, Clone)]
pub struct ElectionConfig {
    pub min_stake_quanta: Quanta,
    pub target_seats: usize,
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            min_stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA,
            target_seats: MINIMUM_GUARD_QUORUM,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElectionOutcome {
    pub epoch: u64,
    pub elected_guards: Vec<PublicKeyBytes>,
    pub candidate_scores: Vec<(PublicKeyBytes, u64)>,
}

#[derive(Debug)]
pub struct EpochElection;

impl EpochElection {
    /// Memilih dewan Guard teratas untuk epoch berikutnya
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika jumlah kandidat yang memenuhi syarat kurang dari
    /// jumlah kursi yang diminta.
    pub fn elect_council(
        epoch: u64,
        candidates: &[CandidateProfile],
        config: &ElectionConfig,
    ) -> Result<ElectionOutcome, GuardError> {
        // 1. Filter awal: Hanya kandidat dengan saldo cukup & tidak di-blacklist
        let mut eligible: Vec<(&CandidateProfile, u64)> = candidates
            .iter()
            .filter(|c| !c.is_blacklisted && c.stake_quanta >= config.min_stake_quanta)
            .map(|c| (c, c.compute_score()))
            .collect();

        if eligible.len() < config.target_seats {
            return Err(GuardError::InsufficientEligibleCandidates {
                eligible: eligible.len(),
                required: config.target_seats,
            });
        }

        // 2. Pengurutan Deterministik:
        //    Prioritas 1: Skor Kontribusi Tertinggi (Descending)
        //    Prioritas 2: Saldo Terkunci Terbanyak (Descending)
        //    Prioritas 3: Byte Kunci Publik (Ascending) sebagai penentu akhir yang absolut
        eligible.sort_by(|(cand_a, score_a), (cand_b, score_b)| {
            score_b
                .cmp(score_a)
                .then_with(|| cand_b.stake_quanta.cmp(&cand_a.stake_quanta))
                .then_with(|| cand_a.validator.cmp(&cand_b.validator))
        });

        // 3. Ambil N kursi teratas
        let selected = &eligible[0..config.target_seats];

        let elected_guards = selected.iter().map(|(c, _)| c.validator).collect();
        let candidate_scores = selected.iter().map(|(c, s)| (c.validator, *s)).collect();

        Ok(ElectionOutcome {
            epoch,
            elected_guards,
            candidate_scores,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn dummy_pubkey(byte: u8) -> PublicKeyBytes {
        [byte; 32]
    }

    #[test]
    fn test_filter_disqualified_candidates() {
        let config = ElectionConfig::default();

        let candidates = vec![
            CandidateProfile {
                validator: dummy_pubkey(1),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA,
                blocks_proposed: 10,
                votes_cast: 50,
                missed_rounds: 0,
                active_epochs: 1,
                is_blacklisted: false,
            },
            CandidateProfile {
                validator: dummy_pubkey(2),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA,
                blocks_proposed: 20,
                votes_cast: 100,
                missed_rounds: 0,
                active_epochs: 2,
                is_blacklisted: false,
            },
            CandidateProfile {
                validator: dummy_pubkey(3),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA,
                blocks_proposed: 15,
                votes_cast: 70,
                missed_rounds: 0,
                active_epochs: 1,
                is_blacklisted: false,
            },
            CandidateProfile {
                validator: dummy_pubkey(4),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA,
                blocks_proposed: 30,
                votes_cast: 150,
                missed_rounds: 1,
                active_epochs: 3,
                is_blacklisted: false,
            },
            CandidateProfile {
                validator: dummy_pubkey(5),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA - 1,
                blocks_proposed: 100,
                votes_cast: 500,
                missed_rounds: 0,
                active_epochs: 5,
                is_blacklisted: false,
            },
            CandidateProfile {
                validator: dummy_pubkey(6),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA * 2,
                blocks_proposed: 100,
                votes_cast: 500,
                missed_rounds: 0,
                active_epochs: 5,
                is_blacklisted: true,
            },
        ];

        let result = EpochElection::elect_council(1, &candidates, &config);
        assert_eq!(
            result.unwrap_err(),
            GuardError::InsufficientEligibleCandidates {
                eligible: 4,
                required: 5,
            }
        );
    }

    #[test]
    fn test_election_ranking_and_tie_breaking() {
        let config = ElectionConfig::default();

        let mut candidates = Vec::new();
        for i in 1..=6 {
            candidates.push(CandidateProfile {
                validator: dummy_pubkey(i),
                stake_quanta: DEFAULT_MIN_GUARD_STAKE_QUANTA,
                blocks_proposed: u64::from(i) * 10,
                votes_cast: u64::from(i) * 50,
                missed_rounds: 0,
                active_epochs: 1,
                is_blacklisted: false,
            });
        }

        let outcome = EpochElection::elect_council(1, &candidates, &config)
            .expect("test operation should succeed");

        assert_eq!(outcome.elected_guards.len(), 5);
        assert_eq!(outcome.elected_guards[0], dummy_pubkey(6));
        assert_eq!(outcome.elected_guards[1], dummy_pubkey(5));
        assert_eq!(outcome.elected_guards[2], dummy_pubkey(4));
        assert_eq!(outcome.elected_guards[3], dummy_pubkey(3));
        assert_eq!(outcome.elected_guards[4], dummy_pubkey(2));
        assert!(!outcome.elected_guards.contains(&dummy_pubkey(1)));
    }
}
