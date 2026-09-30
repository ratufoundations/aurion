#![forbid(unsafe_code)]

pub mod council;
pub mod election;
pub mod error;
pub mod evidence;
pub mod evidence_ledger;
pub mod slashing;
pub mod verdict;

pub use council::{GuardCouncil, MINIMUM_GUARD_QUORUM};
pub use election::{
    CandidateProfile, ElectionConfig, ElectionOutcome, EpochElection,
    DEFAULT_MIN_GUARD_STAKE_QUANTA,
};
pub use error::GuardError;
pub use evidence::{RaidEvidence, RehabilitationPetition, ViolationType};
pub use evidence_ledger::{EvidenceContext, ExecutedEvidenceLedger, MAX_EVIDENCE_AGE_BLOCKS};
pub use slashing::{
    SlashAllocation, SlashCalculator, ViolationSeverity, BPS_SCALE, BURN_RATE_BPS,
    REPORTER_REWARD_BPS, SEVERE_SLASH_BPS, TREASURY_RATE_BPS,
};
pub use verdict::{BlacklistVerdict, PardonVerdict};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use aurion_criptografi::Keypair;
    use std::collections::BTreeMap;

    #[test]
    fn test_guard_raid_blacklist_and_pardon_lifecycle() {
        // 1. Inisialisasi 5 Guard independen
        let guard_keys: Vec<Keypair> = (0..5).map(|_| Keypair::generate()).collect();
        let guard_pks: Vec<aurion_criptografi::PublicKeyBytes> = guard_keys
            .iter()
            .map(aurion_criptografi::Keypair::public_key_bytes)
            .collect();

        let rogue_validator = Keypair::generate().public_key_bytes();

        let mut council = GuardCouncil::new(guard_pks).expect("test operation should succeed");
        assert_eq!(council.total_guards(), 5);

        // 2. Razia: Bukti validator melakukan Double-Signing pada blok 142
        let evidence = RaidEvidence::new(
            rogue_validator,
            ViolationType::DoubleSigning,
            142,
            b"EQUIVOCATION_PAYLOAD_PROOF_BYTES",
        );

        let mut blacklist_verdict = BlacklistVerdict {
            evidence,
            signatures: BTreeMap::new(),
        };
        let digest = blacklist_verdict.digest();

        // 3. Skenario: Hanya 4 dari 5 Guard yang tanda tangan -> HARUS GAGAL
        for k in &guard_keys[0..4] {
            blacklist_verdict
                .signatures
                .insert(k.public_key_bytes(), k.sign(&digest));
        }

        let fail_res = council.execute_blacklist(&blacklist_verdict);
        assert_eq!(
            fail_res.unwrap_err(),
            GuardError::UnanimousConsentNotMet {
                collected: 4,
                required: 5
            }
        );
        assert!(!council.is_blacklisted(&rogue_validator));

        // 4. Guard ke-5 ikut tanda tangan (5/5 sepakat) -> SUKSES DIPUTUS
        blacklist_verdict.signatures.insert(
            guard_keys[4].public_key_bytes(),
            guard_keys[4].sign(&digest),
        );

        council
            .execute_blacklist(&blacklist_verdict)
            .expect("test operation should succeed");
        assert!(council.is_blacklisted(&rogue_validator));

        // 5. REHABILITASI SPORTIF:
        // Validator membuktikan nodenya sudah di-patch dan menyinkronkan ulang
        let petition = RehabilitationPetition::new(
            rogue_validator,
            "Node patched with bugfix commit a1f9, verified state synced.",
            1_700_000_100,
        );

        let mut pardon_verdict = PardonVerdict {
            petition,
            signatures: BTreeMap::new(),
        };
        let pardon_digest = pardon_verdict.digest();

        // Ke-5 Guard memverifikasi pembuktian dan menandatangani amnesti
        for k in &guard_keys {
            pardon_verdict
                .signatures
                .insert(k.public_key_bytes(), k.sign(&pardon_digest));
        }

        // Eksekusi pemulihan: Blacklist resmi dihapus
        council
            .execute_pardon(&pardon_verdict)
            .expect("test operation should succeed");
        assert!(!council.is_blacklisted(&rogue_validator));
    }
}
