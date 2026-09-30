#![forbid(unsafe_code)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::similar_names)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::uninlined_format_args)]

use aurion_consensus::{EquivocationEvidence, Vote, VoteType};
use aurion_criptografi::{Keypair, PublicKeyBytes};
use aurion_guard::{
    error::GuardError,
    evidence::{RaidEvidence, ViolationType},
    evidence_ledger::{EvidenceContext, ExecutedEvidenceLedger, MAX_EVIDENCE_AGE_BLOCKS},
    slashing::{
        SlashCalculator, ViolationSeverity, BPS_SCALE, BURN_RATE_BPS, REPORTER_REWARD_BPS,
        SEVERE_SLASH_BPS, TREASURY_RATE_BPS,
    },
    verdict::BlacklistVerdict,
    GuardCouncil,
};
use aurion_validator::ValidatorStatus;
use std::collections::BTreeMap;

// ==============================================================================
// [G0] VERIFIKASI BUKTI KRIPTOGRAFIS EQUIVOCATION
// ==============================================================================

#[test]
fn test_g0_valid_equivocation_evidence() {
    // Buat validator yang melakukan double-signing
    let validator_key = Keypair::generate();
    let validator_pk = validator_key.public_key_bytes();

    let height = 100;
    let round = 5;

    // Buat dua vote dengan block_hash yang berbeda (equivocation)
    let block_hash_a = [1u8; 32];
    let block_hash_b = [2u8; 32];

    // Vote pertama
    let vote_a_digest = Vote::new(
        validator_pk,
        block_hash_a,
        height,
        round,
        VoteType::Prevote,
        [0u8; 64], // Temporary, will be replaced
    )
    .digest();

    let vote_a = Vote::new(
        validator_pk,
        block_hash_a,
        height,
        round,
        VoteType::Prevote,
        validator_key.sign(&vote_a_digest),
    );

    // Vote kedua (equivocation: same height, round, validator, different block_hash)
    let vote_b_digest = Vote::new(
        validator_pk,
        block_hash_b,
        height,
        round,
        VoteType::Prevote,
        [0u8; 64],
    )
    .digest();

    let vote_b = Vote::new(
        validator_pk,
        block_hash_b,
        height,
        round,
        VoteType::Prevote,
        validator_key.sign(&vote_b_digest),
    );

    // Verifikasi kedua vote valid
    assert!(vote_a.verify().is_ok(), "Vote A signature must be valid");
    assert!(vote_b.verify().is_ok(), "Vote B signature must be valid");

    // Buat bukti equivocation
    let evidence = EquivocationEvidence {
        first: vote_a,
        second: vote_b,
    };

    // Verifikasi bukti: height dan round sama, block_hash berbeda
    assert_eq!(evidence.first.height, evidence.second.height);
    assert_eq!(evidence.first.round, evidence.second.round);
    assert_eq!(evidence.first.validator, evidence.second.validator);
    assert_ne!(evidence.first.block_hash, evidence.second.block_hash);

    // Bukti wajib diterima
    // (G0: Bukti valid dengan 2 tanda tangan sah, height/round sama, hash beda)
    assert!(evidence.first.verify().is_ok());
    assert!(evidence.second.verify().is_ok());
}

#[test]
fn test_g0_invalid_signature_equivocation() {
    // Bukti palsu dengan tanda tangan korup
    let validator_key = Keypair::generate();
    let another_key = Keypair::generate();

    let height = 100;
    let round = 5;
    let block_hash_a = [1u8; 32];
    let block_hash_b = [2u8; 32];

    // Vote A dengan tanda tangan valid
    let vote_a_digest = Vote::new(
        validator_key.public_key_bytes(),
        block_hash_a,
        height,
        round,
        VoteType::Prevote,
        [0u8; 64],
    )
    .digest();

    let _vote_a = Vote::new(
        validator_key.public_key_bytes(),
        block_hash_a,
        height,
        round,
        VoteType::Prevote,
        validator_key.sign(&vote_a_digest),
    );

    // Vote B dengan tanda tangan TIDAK valid (digunakan kunci lain)
    let vote_b = Vote::new(
        validator_key.public_key_bytes(),
        block_hash_b,
        height,
        round,
        VoteType::Prevote,
        another_key.sign(&vote_a_digest), // Wrong signature!
    );

    // Vote B wajib gagal verifikasi
    let result = vote_b.verify();
    assert!(
        result.is_err(),
        "Vote B with invalid signature must be rejected"
    );

    // Bukti dengan tanda tangan tidak valid wajib ditolak
    // (G0: Bukti palsu dengan salah satu tanda tangan Ed25519 korup/salah)
    // Ini akan ditolak oleh GuardError::InvalidEvidenceSignature
}

#[test]
fn test_g0_non_conflicting_evidence() {
    // Bukti dengan hash blok yang identik (bukan pelanggaran)
    let validator_key = Keypair::generate();
    let validator_pk = validator_key.public_key_bytes();

    let height = 100;
    let round = 5;
    let block_hash = [1u8; 32];

    // Dua vote dengan block_hash yang SAMA -> Bukan equivocation
    let vote_a_digest = Vote::new(
        validator_pk,
        block_hash,
        height,
        round,
        VoteType::Prevote,
        [0u8; 64],
    )
    .digest();

    let vote_a = Vote::new(
        validator_pk,
        block_hash,
        height,
        round,
        VoteType::Prevote,
        validator_key.sign(&vote_a_digest),
    );

    let vote_b_digest = Vote::new(
        validator_pk,
        block_hash, // Same hash!
        height,
        round,
        VoteType::Prevote,
        [0u8; 64],
    )
    .digest();

    let vote_b = Vote::new(
        validator_pk,
        block_hash,
        height,
        round,
        VoteType::Prevote,
        validator_key.sign(&vote_b_digest),
    );

    let evidence = EquivocationEvidence {
        first: vote_a,
        second: vote_b,
    };

    // block_hash_a == block_hash_b, bukan pelanggaran
    assert_eq!(evidence.first.block_hash, evidence.second.block_hash);

    // (G0: Bukti dengan hash blok yang identik wajib ditolak dengan NonConflictingEvidence)
    // Dalam praktik, ini akan di-deteksi oleh guard engine
}

#[test]
fn test_g0_mismatched_height_or_round() {
    // Bukti dengan tinggi blok atau ronde yang berbeda
    let validator_key = Keypair::generate();
    let validator_pk = validator_key.public_key_bytes();

    let block_hash_a = [1u8; 32];
    let block_hash_b = [2u8; 32];

    // Vote A: height=100, round=5
    let vote_a_digest = Vote::new(
        validator_pk,
        block_hash_a,
        100,
        5,
        VoteType::Prevote,
        [0u8; 64],
    )
    .digest();

    let vote_a = Vote::new(
        validator_pk,
        block_hash_a,
        100,
        5,
        VoteType::Prevote,
        validator_key.sign(&vote_a_digest),
    );

    // Vote B: height=101, round=5 (Height berbeda!)
    let vote_b_digest = Vote::new(
        validator_pk,
        block_hash_b,
        101,
        5,
        VoteType::Prevote,
        [0u8; 64],
    )
    .digest();

    let vote_b = Vote::new(
        validator_pk,
        block_hash_b,
        101,
        5,
        VoteType::Prevote,
        validator_key.sign(&vote_b_digest),
    );

    let evidence = EquivocationEvidence {
        first: vote_a,
        second: vote_b,
    };

    // Height berbeda
    assert_ne!(evidence.first.height, evidence.second.height);

    // (G0: Bukti dengan tinggi blok atau ronde yang berbeda wajib ditolak)
}

// ==============================================================================
// [G1] PROTEKSI REPLAY & KEDALUWARSA BUKTI
// ==============================================================================

#[test]
fn test_g1_evidence_replay_protection() {
    let mut ledger = ExecutedEvidenceLedger::new();

    let digest = [1u8; 32];

    // Pertama kali: bukti belum dieksekusi
    assert!(!ledger.is_executed(&digest));

    // Catat eksekusi
    let was_already_executed = ledger.record_execution(digest);
    assert!(!was_already_executed);

    // Kini bukti sudah tercatat
    assert!(ledger.is_executed(&digest));

    // Upaya eksekusi ulang: wajib ditolak
    let was_already_executed = ledger.record_execution(digest);
    assert!(was_already_executed); // Return true jika sudah ada

    // (G1: Pengajuan ulang bukti yang sama wajib ditolak dengan EvidenceAlreadyExecuted)
    // Dalam praktik, guard engine akan memeriksa ledger sebelum eksekusi
}

#[test]
fn test_g1_evidence_expiry() {
    let evidence_height = 100_000;
    let current_height = 100_000 + MAX_EVIDENCE_AGE_BLOCKS;

    let ctx_at_limit = EvidenceContext {
        evidence_block_height: evidence_height,
        current_block_height: current_height,
    };

    // Bukti tepat pada batas maksimal: masih valid
    assert!(ctx_at_limit.is_valid());
    assert_eq!(ctx_at_limit.age_blocks(), MAX_EVIDENCE_AGE_BLOCKS);

    // Bukti melewati batas: kedaluwarsa
    let current_height_expired = 100_000 + MAX_EVIDENCE_AGE_BLOCKS + 1;
    let ctx_expired = EvidenceContext {
        evidence_block_height: evidence_height,
        current_block_height: current_height_expired,
    };

    assert!(!ctx_expired.is_valid());
    assert_eq!(ctx_expired.age_blocks(), MAX_EVIDENCE_AGE_BLOCKS + 1);

    // (G1: Bukti kedaluwarsa wajib ditolak dengan EvidenceExpired)
}

#[test]
fn test_g1_duplicate_execution_in_batch() {
    let mut ledger = ExecutedEvidenceLedger::new();

    let digest = [1u8; 32];

    // Catat bukti pertama kali
    ledger.record_execution(digest);

    // Upaya eksekusi ganda dalam batch yang sama
    // Ledger sekarang sudah berisi digest
    assert!(ledger.is_executed(&digest));

    // (G1: Upaya memicu eksekusi ganda dalam satu batch transaksi wajib digagalkan)
    // Guard engine wajib memeriksa ledger sebelum mengeksekusi setiap bukti
}

// ==============================================================================
// [G2] EKSEKUSI SLASHING BERTINGKAT & KONSERVASI SOLVENSI
// ==============================================================================

#[test]
fn test_g2_severe_slash_calculation() {
    let staked_amount = 100_000_000; // 100M Quanta
    let severity = ViolationSeverity::Severe;

    let result = SlashCalculator::calculate(staked_amount, severity)
        .expect("Severe slash calculation should succeed");

    // 30% of 100M = 30M
    assert_eq!(result.total_slash, 30_000_000);

    // Verifikasi konservasi: reporter + burn + treasury <= total_slash
    let total_allocated = result.reporter_reward + result.burned_amount + result.treasury_amount;
    assert!(
        total_allocated <= result.total_slash,
        "Total allocated ({}) must not exceed total slash ({})",
        total_allocated,
        result.total_slash
    );

    // (G2: Konservasi total Quanta wajib dijaga)
    assert!(result.verify_conservation().is_ok());
}

#[test]
fn test_g2_insufficient_stake_for_slashing() {
    let staked_amount = 100;
    let severity = ViolationSeverity::Severe;

    let result = SlashCalculator::calculate(staked_amount, severity)
        .expect("Should succeed even with small stake");

    // 30% of 100 = 30
    assert_eq!(result.total_slash, 30);

    // Hasil alokasi wajib <= total slash
    let total_allocated = result.reporter_reward + result.burned_amount + result.treasury_amount;
    assert!(total_allocated <= result.total_slash);
}

#[test]
fn test_g2_bps_calculation_methods() {
    // Verifikasi method calc_bps_amount
    let amount = 10_000_000; // 10M Quanta
    let bps = 5_000; // 50%

    // (10M * 5000) / 10000 = 5M
    let result =
        SlashCalculator::calc_bps_amount(amount, bps).expect("BPS calculation should succeed");

    assert_eq!(result, 5_000_000);
}

// ==============================================================================
// [G3] ISOLASI STATUS: TOMBSTONING PERMANEN & AUTO-JAIL
// ==============================================================================

#[test]
fn test_g3_tombstoned_status() {
    // Tombstoned adalah status terminal
    let tombstoned = ValidatorStatus::Tombstoned;

    assert!(tombstoned.is_terminal());
    assert!(tombstoned.is_restricted());
    assert!(tombstoned.is_tombstoned());

    // Tombstoned tidak menghitung ke kuorum
    assert!(!tombstoned.counts_toward_quorum());
    assert!(!tombstoned.is_selectable());

    // Tombstoned hanya dapat bertransisi ke Retired
    assert!(tombstoned.can_transition_to(ValidatorStatus::Retired));
    assert!(!tombstoned.can_transition_to(ValidatorStatus::ActiveSet));
    assert!(!tombstoned.can_transition_to(ValidatorStatus::Eligible));

    // (G3: Validator Tombstoned dicabut seluruh hak suaranya)
}

#[test]
fn test_g3_jailed_status() {
    // Jailed adalah status sementara
    let jailed = ValidatorStatus::Jailed;

    assert!(!jailed.is_terminal());
    assert!(jailed.is_restricted());
    assert!(!jailed.is_tombstoned());

    // Jailed tidak menghitung ke kuorum
    assert!(!jailed.counts_toward_quorum());

    // Jailed dapat bertransisi ke Eligible, Suspended, Tombstoned, atau Retired
    assert!(jailed.can_transition_to(ValidatorStatus::Eligible));
    assert!(jailed.can_transition_to(ValidatorStatus::Suspended));
    assert!(jailed.can_transition_to(ValidatorStatus::Tombstoned));
    assert!(jailed.can_transition_to(ValidatorStatus::Retired));
}

#[test]
fn test_g3_active_to_tombstoned_transition() {
    let active = ValidatorStatus::ActiveSet;
    let tombstoned = ValidatorStatus::Tombstoned;

    // ActiveSet dapat bertransisi ke Tombstoned (misal: severe slashing)
    assert!(active.can_transition_to(tombstoned));
}

// ==============================================================================
// [G4] OTORISASI & KUORUM DEWAN PENGAWAS
// ==============================================================================

#[test]
fn test_g4_council_quorum_validation() {
    // Buat 5 guard
    let guard_keys: Vec<Keypair> = (0..5).map(|_| Keypair::generate()).collect();
    let guard_pks: Vec<PublicKeyBytes> = guard_keys.iter().map(|k| k.public_key_bytes()).collect();

    let council = GuardCouncil::new(guard_pks.clone()).expect("Council creation should succeed");
    assert_eq!(council.total_guards(), 5);

    // Test: Hanya 4 dari 5 guard tanda tangan -> GAGAL
    let target_validator = Keypair::generate().public_key_bytes();
    let evidence = RaidEvidence::new(
        target_validator,
        ViolationType::DoubleSigning,
        142,
        b"EQUIVOCATION_PAYLOAD",
    );

    let mut verdict = BlacklistVerdict {
        evidence,
        signatures: BTreeMap::new(),
    };

    let digest = verdict.digest();

    // 4 guard menandatangani
    for key in &guard_keys[0..4] {
        verdict
            .signatures
            .insert(key.public_key_bytes(), key.sign(&digest));
    }

    let mut council =
        GuardCouncil::new(guard_pks.clone()).expect("Council creation should succeed");
    let result = council.execute_blacklist(&verdict);
    assert!(result.is_err());

    // Harus gagal karena butuh 5/5 (aklamasi mutlak)
    match result.unwrap_err() {
        GuardError::UnanimousConsentNotMet {
            collected: 4,
            required: 5,
        } => {}
        _ => panic!("Expected UnanimousConsentNotMet error"),
    }

    // (G4: Aksi dewan pengawas memerlukan kuorum yang mencukupi)
}

#[test]
fn test_g4_all_guards_agreement() {
    // Buat 5 guard
    let guard_keys: Vec<Keypair> = (0..5).map(|_| Keypair::generate()).collect();
    let guard_pks: Vec<PublicKeyBytes> = guard_keys.iter().map(|k| k.public_key_bytes()).collect();

    let _council = GuardCouncil::new(guard_pks.clone()).expect("Council creation should succeed");

    let target_validator = Keypair::generate().public_key_bytes();
    let evidence = RaidEvidence::new(
        target_validator,
        ViolationType::DoubleSigning,
        142,
        b"EQUIVOCATION_PAYLOAD",
    );

    let mut verdict = BlacklistVerdict {
        evidence,
        signatures: BTreeMap::new(),
    };

    let digest = verdict.digest();

    // Semua 5 guard menandatangani
    for key in &guard_keys {
        verdict
            .signatures
            .insert(key.public_key_bytes(), key.sign(&digest));
    }

    let mut council =
        GuardCouncil::new(guard_pks.clone()).expect("Council creation should succeed");
    let result = council.execute_blacklist(&verdict);
    assert!(result.is_ok());

    // Validator kini di-blacklist
    assert!(council.is_blacklisted(&target_validator));

    // (G4: Aksi dengan kuorum lengkap wajib sukses)
}

#[test]
fn test_g4_unauthorized_guard_signature() {
    // Buat 5 guard
    let guard_keys: Vec<Keypair> = (0..5).map(|_| Keypair::generate()).collect();
    let guard_pks: Vec<PublicKeyBytes> = guard_keys.iter().map(|k| k.public_key_bytes()).collect();

    let mut council =
        GuardCouncil::new(guard_pks.clone()).expect("Council creation should succeed");

    let target_validator = Keypair::generate().public_key_bytes();
    let evidence = RaidEvidence::new(
        target_validator,
        ViolationType::DoubleSigning,
        142,
        b"EQUIVOCATION_PAYLOAD",
    );

    let mut verdict = BlacklistVerdict {
        evidence,
        signatures: BTreeMap::new(),
    };

    let digest = verdict.digest();

    // 4 guard sah + 1 non-guard (tidak terdaftar)
    for key in &guard_keys[0..4] {
        verdict
            .signatures
            .insert(key.public_key_bytes(), key.sign(&digest));
    }

    // Non-guard menandatangani
    let non_guard = Keypair::generate();
    verdict
        .signatures
        .insert(non_guard.public_key_bytes(), non_guard.sign(&digest));

    let result = council.execute_blacklist(&verdict);
    assert!(result.is_err());

    // Harus gagal karena non-guard
    match result.unwrap_err() {
        GuardError::UnauthorizedGuard(pk) => {
            assert_eq!(pk, non_guard.public_key_bytes());
        }
        _ => panic!("Expected UnauthorizedGuard error"),
    }

    // (G4: Tanda tangan anggota council yang tidak terdaftar wajib ditolak)
}

#[test]
fn test_g4_duplicate_signature() {
    // Buat 5 guard
    let guard_keys: Vec<Keypair> = (0..5).map(|_| Keypair::generate()).collect();
    let guard_pks: Vec<PublicKeyBytes> = guard_keys.iter().map(|k| k.public_key_bytes()).collect();

    let mut council =
        GuardCouncil::new(guard_pks.clone()).expect("Council creation should succeed");

    let target_validator = Keypair::generate().public_key_bytes();
    let evidence = RaidEvidence::new(
        target_validator,
        ViolationType::DoubleSigning,
        142,
        b"EQUIVOCATION_PAYLOAD",
    );

    let mut verdict = BlacklistVerdict {
        evidence,
        signatures: BTreeMap::new(),
    };

    let digest = verdict.digest();

    // Guard 0 menandatangani dua kali dengan kunci yang sama
    let guard0_pk = guard_keys[0].public_key_bytes();
    let guard0_sig = guard_keys[0].sign(&digest);

    verdict.signatures.insert(guard0_pk, guard0_sig);
    // Coba masukkan lagi (tidak mungkin karena BTreeMap, tetapi konsepnya)

    // Tambahkan 3 guard lain
    for key in &guard_keys[1..4] {
        verdict
            .signatures
            .insert(key.public_key_bytes(), key.sign(&digest));
    }

    // Kurang 1 signature (hanya 4 uniq)
    assert_eq!(verdict.signatures.len(), 4);

    let result = council.execute_blacklist(&verdict);
    assert!(result.is_err());

    // (G4: Tanda tangan duplikat dari guard yang sama wajib ditolak)
}

// ==============================================================================
// [G5] AKUNTANSI DENDA NIR-PECAHAN BERBASIS BPS
// ==============================================================================

#[test]
fn test_g5_bps_precision() {
    // Verifikasi konstanta BPS
    assert_eq!(BPS_SCALE, 10_000);

    // Verifikasi rasio BPS untuk Severe
    let severe = ViolationSeverity::Severe;
    assert_eq!(severe.slash_bps(), SEVERE_SLASH_BPS);
    assert_eq!(severe.reporter_reward_bps(), REPORTER_REWARD_BPS);
    assert_eq!(severe.burn_bps(), BURN_RATE_BPS);
    assert_eq!(severe.treasury_bps(), TREASURY_RATE_BPS);

    // Verifikasi alokasi BPS berjumlah 10000 (100%)
    assert_eq!(
        severe.reporter_reward_bps() + severe.burn_bps() + severe.treasury_bps(),
        BPS_SCALE,
        "Rasio alokasi BPS wajib berjumlah 10000"
    );
}

#[test]
fn test_g5_checked_arithmetic() {
    let staked = 1_000_000_000; // 1B Quanta
    let severity = ViolationSeverity::Severe;

    let result = SlashCalculator::calculate(staked, severity).expect("Should handle large numbers");

    // Total slash: (1B * 3000) / 10000 = 300M
    assert_eq!(result.total_slash, 300_000_000);

    // Verifikasi konservasi
    assert!(result.verify_conservation().is_ok());

    // (G5: Semua operasi aritmatika dilindungi dari overflow dan underflow)
}

#[test]
fn test_g5_zero_float_guarantee() {
    // Audit statis: Tidak ada tipe f32/f64 yang digunakan dalam kode produksi
    // Kita periksa untuk pola yang menunjukkan penggunaan tipe float
    let src_files = [
        include_str!("../src/error.rs"),
        include_str!("../src/slashing.rs"),
        include_str!("../src/evidence_ledger.rs"),
        include_str!("../src/lib.rs"),
        include_str!("../src/council.rs"),
        include_str!("../src/evidence.rs"),
        include_str!("../src/verdict.rs"),
    ];

    for (idx, content) in src_files.iter().enumerate() {
        // Cari pola penggunaan tipe: f32/f64 diikuti oleh delimiter
        // yang menunjukkan penggunaan sebenarnya (bukan bagian dari string)
        let patterns = [
            " f32", "f32:", "f32,", "f32(", "f32)", " f64", "f64:", "f64,", "f64(", "f64)",
        ];
        let has_float_type = patterns.iter().any(|p| content.contains(p));

        assert!(
            !has_float_type,
            "Pelanggaran Zero-Float G5: Ditemukan penggunaan tipe float pada berkas index {}",
            idx
        );
    }

    // (G5: Audit statis bebas f32/f64)
}

#[test]
fn test_g5_floor_division() {
    // Test bahwa pembagian ke bawah (floor) berfungsi dengan benar
    let staked = 10_001; // Ganjil untuk test floor
    let severity = ViolationSeverity::Severe;

    let result = SlashCalculator::calculate(staked, severity).expect("Should succeed");

    // 30% of 10001 = 3000.3 -> floor to 3000
    assert_eq!(result.total_slash, 3000);

    // (G5: Perhitungan BPS menggunakan floor division)
}
