#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

//! Invariant test suite modul `aurion-validator` (V0–V5).
//!
//! Setiap test mengunci satu invarian siklus hidup validator: gerbang
//! penerimaan (`PoP` + stake + peran akun), pipeline probation, rotasi epoch
//! deterministik, pelacakan liveness dengan penahanan otomatis, integritas
//! endorsement anti-Sybil, dan aritmetika basis poin nir-pecahan.

use aurion_account::{
    derive_account_id, AccountId, Role, RolePromotion, SovereignAccount, MIN_ACCOUNT_RESERVE_QUANTA,
};
use aurion_criptografi::Keypair;
use aurion_validator::{
    accumulate_weight, effective_stake_quanta, meets_bps_quorum, select_active_set, slash_amount,
    uptime_bps, BlockReport, EndorsementLedger, EpochSchedule, ProofOfPossession, SuspensionReason,
    UnjailRequest, ValidatorError, ValidatorLifecycle, ValidatorPolicy, ValidatorRecord,
    ValidatorStatus, ALL_VALIDATOR_STATUSES, BPS_DENOMINATOR, DEFAULT_PROBATION_WINDOW_BLOCKS,
    EPOCH_LENGTH_BLOCKS, FULL_PERFORMANCE_BPS, MAX_ACTIVE_ENDORSEMENTS, MAX_ACTIVE_VALIDATORS,
    MAX_MISSED_BLOCKS_THRESHOLD, MIN_ENDORSER_TENURE_BLOCKS, MIN_VALIDATOR_STAKE_QUANTA,
    PROBATION_FLOOR_DIVISOR, SEVERE_SLASH_RATE_BPS, SUSPENSION_COOLDOWN_BLOCKS,
    UNJAIL_COOLDOWN_BLOCKS,
};
use std::collections::BTreeMap;

/// Panjang jendela probation pada skenario uji (`K = 100` blok, contoh V1).
const PROBATION_BLOCKS: u64 = 100;

/// Panjang epoch kecil agar rotasi mudah diuji pada batas blok.
const EPOCH_BLOCKS: u64 = 10;

/// Tinggi bukti (evidence) semu yang memicu karantina ireversibel pada uji.
const TOMBSTONE_EVIDENCE_HEIGHT: u64 = 31;

/// Kebijakan uji dengan kapasitas aktif 3 kursi dan kuota endorsement aktif.
fn test_policy() -> ValidatorPolicy {
    ValidatorPolicy {
        probation_blocks: PROBATION_BLOCKS,
        probation_max_miss_streak: 5,
        max_missed_blocks: MAX_MISSED_BLOCKS_THRESHOLD,
        unjail_cooldown_blocks: 20,
        suspension_cooldown_blocks: 50,
        max_active_validators: 3,
        epoch_length_blocks: EPOCH_BLOCKS,
        max_active_endorsements: MAX_ACTIVE_ENDORSEMENTS,
        min_endorser_tenure_blocks: 20,
    }
}

fn key(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

/// Akun berdaulat ber-peran `ValidatorCandidate` dengan komitmen stake.
fn candidate_account(seed: u8, stake_quanta: u64) -> (Keypair, SovereignAccount) {
    let master = key(seed);
    let account_id = derive_account_id(&master.public_key_bytes());
    let mut account = SovereignAccount::register(
        account_id,
        master.public_key_bytes(),
        0,
        MIN_ACCOUNT_RESERVE_QUANTA,
    )
    .expect("registrasi akun berdaulat");
    let mut promotion = RolePromotion {
        account: account_id,
        new_role: Role::ValidatorCandidate,
        stake_quanta,
        nonce: account.nonce,
        signature: [0_u8; 64],
    };
    promotion.signature = master.sign(&promotion.digest());
    account
        .promote_to_candidate(&promotion, &master.public_key_bytes())
        .expect("promosi kandidat validator");
    (master, account)
}

/// Terbitkan `PoP` atas nama akun dan daftarkan sebagai kandidat validator.
fn register_validator(
    lifecycle: &mut ValidatorLifecycle,
    account: &SovereignAccount,
    consensus_key: &Keypair,
    current_block: u64,
) -> Result<(), ValidatorError> {
    let proof = ProofOfPossession::issue(account.account_id, consensus_key);
    lifecycle.register(account, &proof, current_block)
}

/// Catat blok bertanda tangan sampai syarat jendela probation terpenuhi.
fn drive_probation(lifecycle: &mut ValidatorLifecycle, account: &AccountId) {
    let required = lifecycle.probation_requirement(account);
    let mut block = 0;
    while block < required {
        lifecycle
            .report_block(account, block, true)
            .expect("pelaporan blok probation");
        block = block.saturating_add(1);
    }
}

/// Daftarkan validator, jatuhkan beberapa pelanggaran, lalu luluskan probation.
fn onboard(
    lifecycle: &mut ValidatorLifecycle,
    seed: u8,
    stake_quanta: u64,
    misses_before_graduation: u64,
) -> (Keypair, SovereignAccount) {
    let consensus_key = key(seed.saturating_add(100));
    let (master, account) = candidate_account(seed, stake_quanta);
    register_validator(lifecycle, &account, &consensus_key, 0).expect("penerimaan validator");
    for miss in 0..misses_before_graduation {
        lifecycle
            .report_block(&account.account_id, miss, false)
            .expect("pelanggaran liveness probation");
    }
    drive_probation(lifecycle, &account.account_id);
    assert_eq!(
        status_of(lifecycle, &account.account_id),
        ValidatorStatus::Eligible
    );
    (master, account)
}

fn status_of(lifecycle: &ValidatorLifecycle, account: &AccountId) -> ValidatorStatus {
    lifecycle
        .record(account)
        .expect("rekaman validator terdaftar")
        .status
}

/// Jalankan rotasi epoch dan kembalikan himpunan validator aktif hasilnya.
fn rotate(lifecycle: &mut ValidatorLifecycle, height: u64) -> Vec<AccountId> {
    let rotation = lifecycle.rotate_epoch(height).expect("rotasi epoch sah");
    assert_eq!(rotation.boundary_height, height);
    rotation.active_set
}

/// Rangkaian bita kanonikal daftar akun untuk perbandingan byte-per-byte.
fn flatten(accounts: &[AccountId]) -> Vec<u8> {
    accounts
        .iter()
        .flat_map(|account| account.iter().copied())
        .collect()
}

// ==========================================================================
// V0 — Gerbang penerimaan: peran, ambang stake, dan PoP
// ==========================================================================

#[test]
fn v0_admission_enforces_candidate_role_and_minimum_stake() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    assert!(lifecycle.is_empty());

    // Peran belum kandidat -> UnauthorizedRole, tanpa mutasi state.
    let plain_master = key(9);
    let plain_id = derive_account_id(&plain_master.public_key_bytes());
    let plain = SovereignAccount::register(
        plain_id,
        plain_master.public_key_bytes(),
        0,
        MIN_ACCOUNT_RESERVE_QUANTA,
    )
    .expect("registrasi akun polos");
    match register_validator(&mut lifecycle, &plain, &key(10), 0) {
        Err(ValidatorError::UnauthorizedRole { expected, actual }) => {
            assert_eq!(expected, Role::ValidatorCandidate);
            assert_eq!(actual, Role::StandardUser);
        }
        other => panic!("peran non-kandidat harus ditolak, bukan {other:?}"),
    }
    assert!(lifecycle.is_empty());

    // Stake di bawah ambang (drift defensif) -> InsufficientStake.
    let (_master, mut underfunded) = candidate_account(4, MIN_VALIDATOR_STAKE_QUANTA);
    underfunded.staked_quanta = MIN_VALIDATOR_STAKE_QUANTA - 1;
    match register_validator(&mut lifecycle, &underfunded, &key(5), 0) {
        Err(ValidatorError::InsufficientStake { provided, required }) => {
            assert_eq!(provided, MIN_VALIDATOR_STAKE_QUANTA - 1);
            assert_eq!(required, MIN_VALIDATOR_STAKE_QUANTA);
        }
        other => panic!("stake di bawah ambang harus ditolak, bukan {other:?}"),
    }
    assert!(lifecycle.is_empty());

    // Penerimaan sah: Probation, belum ikut dihitung kuorum maupun seleksi.
    let (_master, funded) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &funded, &key(100), 0).expect("kandidat sah diterima");
    assert_eq!(
        status_of(&lifecycle, &funded.account_id),
        ValidatorStatus::Probation
    );
    assert_eq!(lifecycle.len(), 1);
    assert_eq!(lifecycle.active_validator_count(), 0);
    assert!(lifecycle.active_set().is_empty());
    assert_eq!(lifecycle.quorum_weight().expect("tanpa luapan"), 0);
    assert_eq!(
        lifecycle.total_stake_quanta().expect("tanpa luapan"),
        MIN_VALIDATOR_STAKE_QUANTA
    );
}

#[test]
fn v0_admission_rejects_duplicate_keys_and_forged_pops_atomically() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master_a, account_a) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA);
    let consensus = key(100);
    register_validator(&mut lifecycle, &account_a, &consensus, 0).expect("penerimaan pertama");
    let digest = lifecycle.state_digest();

    // Kunci konsensus kedua dipakai pihak lain -> DuplicateConsensusKey.
    let (_master_b, account_b) = candidate_account(2, MIN_VALIDATOR_STAKE_QUANTA);
    match register_validator(&mut lifecycle, &account_b, &consensus, 1) {
        Err(ValidatorError::DuplicateConsensusKey(key_bytes)) => {
            assert_eq!(key_bytes, consensus.public_key_bytes());
        }
        other => panic!("duplikasi kunci konsensus harus ditolak, bukan {other:?}"),
    }

    // Akun sama mendaftar dua kali -> AlreadyRegistered.
    match register_validator(&mut lifecycle, &account_a, &consensus, 1) {
        Err(ValidatorError::AlreadyRegistered(account)) => {
            assert_eq!(account, account_a.account_id);
        }
        other => panic!("pendaftaran ganda harus ditolak, bukan {other:?}"),
    }

    // PoP terikat pada akun lain (kandidat belum terdaftar) -> ditolak.
    let cross_bound = ProofOfPossession::issue(account_a.account_id, &key(101));
    match lifecycle.register(&account_b, &cross_bound, 1) {
        Err(ValidatorError::InvalidProofOfPossession) => {}
        other => panic!("PoP silang-akun harus ditolak, bukan {other:?}"),
    }

    // PoP yang tanda tangannya atas berkas berbeda -> InvalidProofOfPossession.
    let mut tampered = ProofOfPossession::issue(account_b.account_id, &key(102));
    tampered.signature = key(103).sign(b"digest-berkas-lain");
    match lifecycle.register(&account_b, &tampered, 1) {
        Err(ValidatorError::InvalidProofOfPossession) => {}
        other => panic!("PoP dimalsukan harus ditolak, bukan {other:?}"),
    }

    // Seluruh penolakan atomik: digest state, jumlah rekaman, dan kuorum utuh.
    assert_eq!(lifecycle.state_digest(), digest);
    assert_eq!(lifecycle.len(), 1);
    assert!(lifecycle.active_set().is_empty());
}

#[test]
fn v0_default_policy_matches_protocol_constants_and_rejects_zeroes() {
    // Kebijakan bawaan selaras konstanta protokol dan lolos validasi.
    let defaults = ValidatorPolicy::default();
    defaults.validate().expect("kebijakan bawaan valid");
    assert_eq!(defaults.probation_blocks, DEFAULT_PROBATION_WINDOW_BLOCKS);
    assert_eq!(defaults.epoch_length_blocks, EPOCH_LENGTH_BLOCKS);
    assert_eq!(defaults.max_missed_blocks, MAX_MISSED_BLOCKS_THRESHOLD);
    assert_eq!(
        defaults.probation_max_miss_streak,
        MAX_MISSED_BLOCKS_THRESHOLD
    );
    assert_eq!(defaults.unjail_cooldown_blocks, UNJAIL_COOLDOWN_BLOCKS);
    assert_eq!(
        defaults.suspension_cooldown_blocks,
        SUSPENSION_COOLDOWN_BLOCKS
    );
    assert!(
        defaults.suspension_cooldown_blocks > defaults.unjail_cooldown_blocks,
        "masa tangguh selalu lebih panjang daripada masa unjail"
    );
    assert_eq!(defaults.max_active_endorsements, MAX_ACTIVE_ENDORSEMENTS);
    assert_eq!(
        defaults.min_endorser_tenure_blocks,
        MIN_ENDORSER_TENURE_BLOCKS
    );
    assert_eq!(defaults.max_active_validators, MAX_ACTIVE_VALIDATORS);

    // Ambang nol ditolak: jendela/proyek yang mustahil dipenuhi tidak valid.
    let zero_window = ValidatorPolicy {
        probation_blocks: 0,
        ..ValidatorPolicy::default()
    };
    match zero_window.validate() {
        Err(ValidatorError::InvalidValidatorPolicy { reason }) => {
            assert!(!reason.is_empty());
        }
        other => panic!("kebijakan nol harus ditolak, bukan {other:?}"),
    }
    match ValidatorLifecycle::new(zero_window) {
        Err(ValidatorError::InvalidValidatorPolicy { .. }) => {}
        other => panic!("mesin kebijakan nol harus ditolak, bukan {other:?}"),
    }
}

// ==========================================================================
// V1 — Pipeline probation: jendela K blok, reset liveness, promosi tunggal
// ==========================================================================

#[test]
fn v1_probation_never_counts_until_k_signed_blocks_are_reported() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA);
    let account_id = account.account_id;
    register_validator(&mut lifecycle, &account, &key(100), 0).expect("penerimaan validator");

    // Probation: bobot kuorum nol, tak terpilih, tak bisa dipromosi paksa.
    assert_eq!(
        lifecycle.probation_requirement(&account_id),
        PROBATION_BLOCKS
    );
    assert_eq!(lifecycle.active_validator_count(), 0);
    assert!(lifecycle.active_set().is_empty());
    assert_eq!(
        rotate(&mut lifecycle, EPOCH_BLOCKS),
        Vec::<AccountId>::new()
    );
    match lifecycle.activate(&account_id, EPOCH_BLOCKS) {
        Err(ValidatorError::InvalidStatusTransition { from, to }) => {
            assert_eq!(from, ValidatorStatus::Probation);
            assert_eq!(to, ValidatorStatus::ActiveSet);
        }
        other => panic!("promosi paksa dari probation harus ditolak, bukan {other:?}"),
    }

    // Kelulusan prematur ditolak dengan pengamatan yang tepat.
    match lifecycle.graduate(&account_id) {
        Err(ValidatorError::ProbationNotFinished { observed, required }) => {
            assert_eq!(observed, 0);
            assert_eq!(required, PROBATION_BLOCKS);
        }
        other => panic!("kelulusan prematur harus ditolak, bukan {other:?}"),
    }

    // K-1 blok bertanda tangan belum cukup; blok ke-K meluluskan otomatis.
    let mut block = 0_u64;
    while block + 1 < PROBATION_BLOCKS {
        let outcome = lifecycle
            .report_block(&account_id, block, true)
            .expect("blok probation sah");
        assert!(matches!(outcome, BlockReport::ProbationProgress { .. }));
        assert_eq!(
            status_of(&lifecycle, &account_id),
            ValidatorStatus::Probation
        );
        block += 1;
    }
    let graduation = lifecycle
        .report_block(&account_id, block, true)
        .expect("blok kelulusan");
    assert!(matches!(graduation, BlockReport::Graduated { .. }));
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Eligible
    );
    assert_eq!(lifecycle.graduate(&account_id), Ok(()), "idempoten");

    // Baru setelah batas epoch Eligible naik dan bobot kuorum muncul.
    let rotation = lifecycle
        .rotate_epoch(EPOCH_BLOCKS)
        .expect("rotasi epoch sah");
    assert_eq!(rotation.active_set, vec![account_id]);
    assert_eq!(lifecycle.active_validator_count(), 1);
    assert_eq!(
        lifecycle.quorum_weight().expect("tanpa luapan"),
        MIN_VALIDATOR_STAKE_QUANTA
    );
}

#[test]
fn v1_probation_liveness_violation_resets_window_and_cancels_repeated_faults() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA);
    let account_id = account.account_id;
    register_validator(&mut lifecycle, &account, &key(100), 0).expect("penerimaan validator");

    for block in 0..60 {
        lifecycle
            .report_block(&account_id, block, true)
            .expect("blok probation sah");
    }
    assert_eq!(
        lifecycle
            .record(&account_id)
            .expect("rekaman")
            .probation_blocks_observed,
        60
    );

    // Satu blok terlewat: jendela dimulai ulang, status tetap Probation.
    let reset = lifecycle
        .report_block(&account_id, 60, false)
        .expect("pelanggaran pertama");
    assert_eq!(reset, BlockReport::ProbationWindowReset { miss_streak: 1 });
    let record = lifecycle.record(&account_id).expect("rekaman");
    assert_eq!(record.probation_blocks_observed, 0);
    assert_eq!(record.probation_signed_blocks, 0);
    assert_eq!(record.probation_miss_streak, 1);
    assert_eq!(record.status, ValidatorStatus::Probation);

    // Jendela ter-reset: kelulusan tidak dapat dilompati.
    match lifecycle.graduate(&account_id) {
        Err(ValidatorError::ProbationNotFinished { observed, required }) => {
            assert_eq!(observed, 0);
            assert_eq!(required, PROBATION_BLOCKS);
        }
        other => panic!("kelulusan setelah reset harus ditolak, bukan {other:?}"),
    }

    // Pelanggaran berurutan sampai ambang -> Retired sebagai mutasi terminal.
    let miss_limit = test_policy().probation_max_miss_streak;
    let mut streak = 1_u64;
    let mut height = 61_u64;
    while streak + 1 < miss_limit {
        let report = lifecycle
            .report_block(&account_id, height, false)
            .expect("pelanggaran berikutnya");
        streak += 1;
        assert_eq!(
            report,
            BlockReport::ProbationWindowReset {
                miss_streak: streak
            }
        );
        height += 1;
    }
    match lifecycle.report_block(&account_id, height, false) {
        Err(ValidatorError::ProbationFailed {
            misses,
            observed_blocks,
        }) => {
            assert_eq!(misses, miss_limit);
            assert_eq!(observed_blocks, 0);
        }
        other => panic!("pembatalan probation harus dikembalikan, bukan {other:?}"),
    }
    assert_eq!(status_of(&lifecycle, &account_id), ValidatorStatus::Retired);

    // Status terminal: pelaporan lanjutan ditolak tanpa mengubah state.
    let digest = lifecycle.state_digest();
    match lifecycle.report_block(&account_id, height + 1, true) {
        Err(ValidatorError::NotActiveInConsensus { status }) => {
            assert_eq!(status, ValidatorStatus::Retired);
        }
        other => panic!("validator Retired tidak boleh dilaporkan, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);
}

#[test]
fn v1_transition_matrix_is_exhaustive_and_probation_cannot_jump() {
    // Tabel transisi sah bersifat eksplisit: 7 status, 18 pasangan. Tabel ini
    // adalah rujukan tunggal; `authorize_transition` harus selalu sinkron
    // dengannya. Status `Tombstoned` (pelanggaran kritis) ditambahkan pada
    // commit f617f8f (14->17), dan jalur karantina `Probation -> Tombstoned`
    // untuk double-signing menambah satu pasangan (17->18). `Tombstoned`
    // adalah terminal absolut: `Eligible -> Tombstoned` dibuka sementara
    // `Tombstoned -> Retired` dicabut, sehingga total tetap 18 pasangan.
    let expected_matrix: Vec<(ValidatorStatus, ValidatorStatus)> = vec![
        (ValidatorStatus::Probation, ValidatorStatus::Eligible),
        (ValidatorStatus::Probation, ValidatorStatus::Tombstoned),
        (ValidatorStatus::Probation, ValidatorStatus::Retired),
        (ValidatorStatus::Eligible, ValidatorStatus::ActiveSet),
        (ValidatorStatus::Eligible, ValidatorStatus::Suspended),
        (ValidatorStatus::Eligible, ValidatorStatus::Tombstoned),
        (ValidatorStatus::Eligible, ValidatorStatus::Retired),
        (ValidatorStatus::ActiveSet, ValidatorStatus::Eligible),
        (ValidatorStatus::ActiveSet, ValidatorStatus::Jailed),
        (ValidatorStatus::ActiveSet, ValidatorStatus::Suspended),
        (ValidatorStatus::ActiveSet, ValidatorStatus::Tombstoned),
        (ValidatorStatus::ActiveSet, ValidatorStatus::Retired),
        (ValidatorStatus::Jailed, ValidatorStatus::Eligible),
        (ValidatorStatus::Jailed, ValidatorStatus::Suspended),
        (ValidatorStatus::Jailed, ValidatorStatus::Tombstoned),
        (ValidatorStatus::Jailed, ValidatorStatus::Retired),
        (ValidatorStatus::Suspended, ValidatorStatus::Eligible),
        (ValidatorStatus::Suspended, ValidatorStatus::Retired),
    ];

    let mut allowed: Vec<(ValidatorStatus, ValidatorStatus)> = Vec::new();
    for from in ALL_VALIDATOR_STATUSES {
        for to in ALL_VALIDATOR_STATUSES {
            if from.can_transition_to(to) {
                allowed.push((from, to));
            }
            let expected = if from.can_transition_to(to) {
                Ok(())
            } else {
                Err(ValidatorError::InvalidStatusTransition { from, to })
            };
            assert_eq!(from.authorize_transition(to), expected);
        }
    }
    // Bandingkan himpunan pasangan, bukan hanya jumlahnya, agar selisih
    // transisi masa depan langsung terlihat pada pesan kegagalan.
    assert_eq!(allowed, expected_matrix);
    assert_eq!(allowed.len(), 18);

    // Gerbang status: hanya Eligible/ActiveSet ikut seleksi, hanya ActiveSet
    // berkuorum, dan status buntu adalah Retired serta Tombstoned.
    for status in ALL_VALIDATOR_STATUSES {
        assert_eq!(
            status.is_selectable(),
            matches!(
                status,
                ValidatorStatus::Eligible | ValidatorStatus::ActiveSet
            )
        );
        assert_eq!(
            status.counts_toward_quorum(),
            status == ValidatorStatus::ActiveSet
        );
        assert_eq!(
            status.is_terminal(),
            matches!(
                status,
                ValidatorStatus::Retired | ValidatorStatus::Tombstoned
            )
        );
        assert_eq!(status.is_probation(), status == ValidatorStatus::Probation);
        assert_eq!(
            status.is_restricted(),
            matches!(
                status,
                ValidatorStatus::Jailed | ValidatorStatus::Suspended | ValidatorStatus::Tombstoned
            )
        );
    }

    // Promosi paksa Probation -> ActiveSet ditolak tanpa jejak state.
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA);
    let account_id = account.account_id;
    register_validator(&mut lifecycle, &account, &key(100), 0).expect("penerimaan validator");
    let digest = lifecycle.state_digest();
    match lifecycle.activate(&account_id, EPOCH_BLOCKS) {
        Err(ValidatorError::InvalidStatusTransition { from, to }) => {
            assert_eq!(from, ValidatorStatus::Probation);
            assert_eq!(to, ValidatorStatus::ActiveSet);
        }
        other => panic!("promosi paksa harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Probation
    );
    assert!(lifecycle.active_set().is_empty());
}

// ==========================================================================
// V2 — Rotasi epoch deterministik: batas, urutan, kapasitas
// ==========================================================================

#[test]
fn v2_epoch_rotation_only_executes_on_boundary_heights() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = onboard(&mut lifecycle, 1, MIN_VALIDATOR_STAKE_QUANTA, 0);
    let account_id = account.account_id;
    let digest = lifecycle.state_digest();

    // Ketinggian di luar batas epoch ditolak tanpa mengubah state apa pun.
    match lifecycle.rotate_epoch(EPOCH_BLOCKS - 1) {
        Err(ValidatorError::EpochNotBoundary { height }) => {
            assert_eq!(height, EPOCH_BLOCKS - 1);
        }
        other => panic!("rotasi di luar batas harus ditolak, bukan {other:?}"),
    }
    match lifecycle.activate(&account_id, EPOCH_BLOCKS + 1) {
        Err(ValidatorError::EpochNotBoundary { height }) => {
            assert_eq!(height, EPOCH_BLOCKS + 1);
        }
        other => panic!("aktivasi di luar batas harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Eligible
    );
    assert!(lifecycle.active_set().is_empty());

    // Jadwal aritmetika murni: kelipatan panjang epoch adalah batas, sisanya bukan.
    let schedule = EpochSchedule::new(EPOCH_BLOCKS);
    assert!(schedule.is_boundary(0));
    assert!(schedule.is_boundary(EPOCH_BLOCKS * 7));
    assert!(!schedule.is_boundary(EPOCH_BLOCKS - 1));
    assert_eq!(schedule.epoch_of(EPOCH_BLOCKS * 7), 7);
    assert_eq!(schedule.epoch_of(EPOCH_BLOCKS * 7 - 1), 6);
    // Panjang epoch nol tidak pernah menghasilkan batas (fail-closed).
    let degenerate = EpochSchedule::new(0);
    assert!(!degenerate.is_boundary(0));
    assert_eq!(degenerate.epoch_of(42), 0);

    // Blok nol adalah batas epoch pertama yang sah.
    let rotation = lifecycle.rotate_epoch(0).expect("rotasi blok nol");
    assert_eq!(rotation.epoch, 0);
    assert_eq!(rotation.boundary_height, 0);
    assert_eq!(rotation.active_set, vec![account_id]);
    assert_eq!(rotation.promoted, vec![account_id]);
    assert!(rotation.demoted.is_empty());
}

#[test]
fn v2_selection_is_deterministic_ordered_and_stable_across_epochs() {
    let mut forward = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let mut backward = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let mut accounts: Vec<SovereignAccount> = Vec::new();
    for seed in 1_u8..=5 {
        let stake = MIN_VALIDATOR_STAKE_QUANTA + u64::from(seed);
        let (_master, account) = candidate_account(seed, stake);
        accounts.push(account);
    }
    let consensus_keys: Vec<Keypair> = (0..accounts.len())
        .map(|index| key(100_u8.saturating_add(u8::try_from(index).expect("jumlah kecil"))))
        .collect();

    // Pendaftaran urut maupun terbalik menghasilkan digest state identik.
    for (account, consensus) in accounts.iter().zip(&consensus_keys) {
        register_validator(&mut forward, account, consensus, 0).expect("registrasi maju");
    }
    for (account, consensus) in accounts.iter().zip(&consensus_keys).rev() {
        register_validator(&mut backward, account, consensus, 0).expect("registrasi mundur");
    }
    assert_eq!(forward.state_digest(), backward.state_digest());

    for account in &accounts {
        drive_probation(&mut forward, &account.account_id);
        drive_probation(&mut backward, &account.account_id);
    }
    assert_eq!(forward.state_digest(), backward.state_digest());
    assert_eq!(
        forward
            .selection_preview(EPOCH_BLOCKS)
            .expect("pratinjau maju"),
        backward
            .selection_preview(EPOCH_BLOCKS)
            .expect("pratinjau mundur")
    );

    // Peringkat skor menurun; kapasitas 3 kursi memangkas dua kandidat bawah.
    let rotation = forward
        .rotate_epoch(EPOCH_BLOCKS)
        .expect("rotasi epoch sah");
    assert_eq!(rotation.epoch, 1);
    let active_expected: Vec<AccountId> = accounts
        .iter()
        .rev()
        .take(3)
        .map(|account| account.account_id)
        .collect();
    let ranking_expected: Vec<(AccountId, u64)> = accounts
        .iter()
        .rev()
        .map(|account| (account.account_id, account.staked_quanta))
        .collect();
    assert_eq!(rotation.active_set, active_expected);
    assert_eq!(rotation.ranking, ranking_expected);
    assert!(rotation.demoted.is_empty());
    let mut promoted_sorted = rotation.active_set.clone();
    promoted_sorted.sort_unstable();
    assert_eq!(rotation.promoted, promoted_sorted);

    // Anggota incumbent tetap terpilih pada rotasi berikutnya: tanpa osilasi.
    let second = forward
        .rotate_epoch(2 * EPOCH_BLOCKS)
        .expect("rotasi kedua");
    assert_eq!(second.epoch, 2);
    assert_eq!(second.active_set, active_expected);
    assert!(second.promoted.is_empty());
    assert!(second.demoted.is_empty());
    assert_eq!(forward.active_validator_count(), 3);
    assert_eq!(
        forward.quorum_weight().expect("tanpa luapan"),
        3 * MIN_VALIDATOR_STAKE_QUANTA + 12
    );

    // Pratinjau dan fungsi seleksi murni identik byte-per-byte.
    let preview = forward
        .selection_preview(3 * EPOCH_BLOCKS)
        .expect("pratinjau");
    let mut records: BTreeMap<AccountId, ValidatorRecord> = BTreeMap::new();
    for (account, record) in forward.records() {
        records.insert(*account, record.clone());
    }
    let pure = select_active_set(
        EpochSchedule::new(EPOCH_BLOCKS),
        3 * EPOCH_BLOCKS,
        &records,
        3,
    )
    .expect("seleksi murni");
    assert_eq!(preview, pure);
    assert_eq!(preview.active_set, active_expected);
    assert_eq!(flatten(&preview.active_set), flatten(&pure.active_set));
}

#[test]
fn v2_tie_break_is_account_id_and_demotion_returns_eligible() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_m1, account_1) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA + 6);
    let (_m2, account_2) = candidate_account(2, MIN_VALIDATOR_STAKE_QUANTA + 4);
    let (_m3, account_3) = candidate_account(3, MIN_VALIDATOR_STAKE_QUANTA + 4);
    let (_m4, account_4) = candidate_account(4, MIN_VALIDATOR_STAKE_QUANTA + 3);
    register_validator(&mut lifecycle, &account_1, &key(101), 0).expect("penerimaan 1");
    register_validator(&mut lifecycle, &account_2, &key(102), 0).expect("penerimaan 2");
    register_validator(&mut lifecycle, &account_3, &key(103), 0).expect("penerimaan 3");
    register_validator(&mut lifecycle, &account_4, &key(104), 0).expect("penerimaan 4");
    for account in [&account_1, &account_2, &account_3, &account_4] {
        drive_probation(&mut lifecycle, &account.account_id);
    }

    // Seri skor diputus oleh AccountId menaik: dua stake identik terurut id.
    let (tie_lo, tie_hi) = if account_2.account_id < account_3.account_id {
        (account_2.account_id, account_3.account_id)
    } else {
        (account_3.account_id, account_2.account_id)
    };
    let first = lifecycle
        .rotate_epoch(EPOCH_BLOCKS)
        .expect("rotasi pertama");
    let expected_top: Vec<AccountId> = vec![account_1.account_id, tie_lo, tie_hi];
    assert_eq!(first.active_set, expected_top);
    assert_eq!(
        first.ranking,
        vec![
            (account_1.account_id, MIN_VALIDATOR_STAKE_QUANTA + 6),
            (tie_lo, MIN_VALIDATOR_STAKE_QUANTA + 4),
            (tie_hi, MIN_VALIDATOR_STAKE_QUANTA + 4),
            (account_4.account_id, MIN_VALIDATOR_STAKE_QUANTA + 3),
        ]
    );
    assert!(first.demoted.is_empty());

    // Satu blok terlewat menurunkan skor anggota tertinggal di bawah penantang.
    let miss_report = lifecycle
        .report_block(&tie_hi, EPOCH_BLOCKS + 1, false)
        .expect("blok terlewat");
    assert_eq!(
        miss_report,
        BlockReport::MissRecorded {
            consecutive_missed_blocks: 1
        }
    );

    let second = lifecycle
        .rotate_epoch(2 * EPOCH_BLOCKS)
        .expect("rotasi kedua");
    let expected_second: Vec<AccountId> = vec![account_1.account_id, tie_lo, account_4.account_id];
    assert_eq!(second.active_set, expected_second);
    assert_eq!(second.promoted, vec![account_4.account_id]);
    assert_eq!(second.demoted, vec![tie_hi]);

    // Penurunan menuju Eligible (bukan Jailed) dan kursi tersalin secara utuh.
    assert_eq!(status_of(&lifecycle, &tie_hi), ValidatorStatus::Eligible);
    assert_eq!(
        status_of(&lifecycle, &account_4.account_id),
        ValidatorStatus::ActiveSet
    );
    assert_eq!(lifecycle.active_validator_count(), 3);
    let survivor = lifecycle.record(&tie_lo).expect("anggota bertahan");
    assert_eq!(survivor.active_since_block, EPOCH_BLOCKS);
    let promoted = lifecycle
        .record(&account_4.account_id)
        .expect("penantang terpromosi");
    assert_eq!(promoted.active_since_block, 2 * EPOCH_BLOCKS);
    let demoted = lifecycle.record(&tie_hi).expect("anggota diturunkan");
    assert_eq!(demoted.active_since_block, 0);
    assert_eq!(demoted.consecutive_missed_blocks, 1);
}

// ==========================================================================
// V3 — Liveness: penahanan otomatis, cooldown unjail, masa tangguh
// ==========================================================================

/// Onboard satu validator, aktifkan pada epoch pertama, lalu jatuhkan ke
/// `Jailed` tepat pada ambang blok terlewat. Mengembalikan kunci master,
/// akun, dan ketinggian penahanan.
fn jail_active_validator(
    lifecycle: &mut ValidatorLifecycle,
    seed: u8,
) -> (Keypair, SovereignAccount, u64) {
    let (master, account) = onboard(lifecycle, seed, MIN_VALIDATOR_STAKE_QUANTA, 0);
    assert_eq!(rotate(lifecycle, EPOCH_BLOCKS), vec![account.account_id]);
    let threshold = lifecycle.policy().max_missed_blocks;
    let mut height = EPOCH_BLOCKS + 1;
    let mut missed = 0_u64;
    while missed + 1 < threshold {
        lifecycle
            .report_block(&account.account_id, height, false)
            .expect("blok terlewat");
        missed += 1;
        height += 1;
    }
    let report = lifecycle
        .report_block(&account.account_id, height, false)
        .expect("penahanan otomatis");
    assert!(matches!(report, BlockReport::AutoJailed { .. }));
    assert_eq!(
        status_of(lifecycle, &account.account_id),
        ValidatorStatus::Jailed
    );
    (master, account, height)
}

#[test]
fn v3_active_validator_auto_jails_at_miss_threshold_and_leaves_quorum() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = onboard(&mut lifecycle, 1, MIN_VALIDATOR_STAKE_QUANTA, 0);
    let account_id = account.account_id;
    assert_eq!(rotate(&mut lifecycle, EPOCH_BLOCKS), vec![account_id]);
    assert_eq!(lifecycle.active_validator_count(), 1);
    assert_eq!(
        lifecycle.quorum_weight().expect("tanpa luapan"),
        MIN_VALIDATOR_STAKE_QUANTA
    );

    // Di bawah ambang: blok terlewat dicatat tanpa menahan validator.
    let threshold = test_policy().max_missed_blocks;
    let mut height = EPOCH_BLOCKS + 1;
    for consecutive in 1..threshold {
        let report = lifecycle
            .report_block(&account_id, height, false)
            .expect("blok terlewat");
        assert_eq!(
            report,
            BlockReport::MissRecorded {
                consecutive_missed_blocks: consecutive
            }
        );
        assert_eq!(
            status_of(&lifecycle, &account_id),
            ValidatorStatus::ActiveSet
        );
        height += 1;
    }

    // Tepat pada ambang: penahanan otomatis dan langsung keluar dari kuorum.
    let jailed = lifecycle
        .report_block(&account_id, height, false)
        .expect("penahanan otomatis");
    assert_eq!(
        jailed,
        BlockReport::AutoJailed {
            consecutive_missed_blocks: threshold,
            height
        }
    );
    assert_eq!(status_of(&lifecycle, &account_id), ValidatorStatus::Jailed);
    let record = lifecycle.record(&account_id).expect("rekaman");
    assert_eq!(record.jail_start_block, height);
    assert_eq!(record.consecutive_missed_blocks, threshold);
    assert!(record.status.is_restricted());
    assert_eq!(lifecycle.active_validator_count(), 0);
    assert!(lifecycle.active_set().is_empty());
    assert_eq!(lifecycle.quorum_weight().expect("tanpa luapan"), 0);

    // Validator ditahan tidak boleh dilaporkan maupun dipromosi paksa.
    let digest = lifecycle.state_digest();
    match lifecycle.report_block(&account_id, height + 1, true) {
        Err(ValidatorError::NotActiveInConsensus { status }) => {
            assert_eq!(status, ValidatorStatus::Jailed);
        }
        other => panic!("validator ditahan harus ditolak, bukan {other:?}"),
    }
    match lifecycle.activate(&account_id, 2 * EPOCH_BLOCKS) {
        Err(ValidatorError::InvalidStatusTransition { from, to }) => {
            assert_eq!(from, ValidatorStatus::Jailed);
            assert_eq!(to, ValidatorStatus::ActiveSet);
        }
        other => panic!("aktivasi paksa harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);
}

#[test]
fn v3_unjail_requires_cooldown_authorization_and_fresh_nonce() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (master, account, jail_height) = jail_active_validator(&mut lifecycle, 1);
    let account_id = account.account_id;
    let cooldown = test_policy().unjail_cooldown_blocks;

    // Permohonan dari akun yang tak terdaftar ditolak lebih dulu.
    let stranger = derive_account_id(&key(50).public_key_bytes());
    match lifecycle.request_unjail(&UnjailRequest::issue(stranger, &key(51), 0, 0), 0) {
        Err(ValidatorError::NotRegistered(found)) => assert_eq!(found, stranger),
        other => panic!("akun asing harus ditolak, bukan {other:?}"),
    }

    // Kunci bukan master dan nonce tidak cocok: otorisasi ditolak (anti-replay).
    match lifecycle.request_unjail(
        &UnjailRequest::issue(account_id, &key(52), jail_height, 0),
        jail_height,
    ) {
        Err(ValidatorError::InvalidUnjailAuthorization) => {}
        other => panic!("kunci asing harus ditolak, bukan {other:?}"),
    }
    match lifecycle.request_unjail(
        &UnjailRequest::issue(account_id, &master, jail_height, 7),
        jail_height,
    ) {
        Err(ValidatorError::InvalidUnjailAuthorization) => {}
        other => panic!("nonce salah harus ditolak, bukan {other:?}"),
    }

    // Masa denda blok: satu blok sebelum cooldown penuh masih ditolak.
    let too_early_at = jail_height + cooldown - 1;
    match lifecycle.request_unjail(
        &UnjailRequest::issue(account_id, &master, too_early_at, 0),
        too_early_at,
    ) {
        Err(ValidatorError::UnjailCooldownActive { remaining }) => {
            assert_eq!(remaining, 1);
        }
        other => panic!("permohonan dini harus ditolak, bukan {other:?}"),
    }
    assert_eq!(status_of(&lifecycle, &account_id), ValidatorStatus::Jailed);

    // Permohonan sah tepat setelah cooldown: kembali Eligible, nonce maju.
    let released_at = jail_height + cooldown;
    let valid = UnjailRequest::issue(account_id, &master, released_at, 0);
    lifecycle
        .request_unjail(&valid, released_at)
        .expect("unjail sah");
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Eligible
    );
    let released = lifecycle.record(&account_id).expect("rekaman");
    assert_eq!(released.unjail_nonce, 1);
    assert_eq!(released.jail_start_block, 0);
    assert_eq!(released.consecutive_missed_blocks, 0);
    assert_eq!(lifecycle.active_validator_count(), 0);

    // Saat masih Eligible, permohonan ganda ditolak tanpa mengubah state.
    let digest = lifecycle.state_digest();
    match lifecycle.request_unjail(&valid, released_at) {
        Err(ValidatorError::InvalidStatusTransition { from, to }) => {
            assert_eq!(from, ValidatorStatus::Eligible);
            assert_eq!(to, ValidatorStatus::Eligible);
        }
        other => panic!("unjail ganda harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);

    // Setelah aktif kembali lalu ditahan kedua kali, tanda tangan lama basi.
    let reactivated = rotate(&mut lifecycle, 4 * EPOCH_BLOCKS);
    assert!(reactivated.contains(&account_id));
    let threshold = test_policy().max_missed_blocks;
    let mut height = 4 * EPOCH_BLOCKS + 1;
    let mut missed = 0_u64;
    while missed + 1 < threshold {
        lifecycle
            .report_block(&account_id, height, false)
            .expect("blok terlewat kedua");
        missed += 1;
        height += 1;
    }
    lifecycle
        .report_block(&account_id, height, false)
        .expect("penahanan kedua");
    assert_eq!(status_of(&lifecycle, &account_id), ValidatorStatus::Jailed);

    match lifecycle.request_unjail(&valid, height) {
        Err(ValidatorError::InvalidUnjailAuthorization) => {}
        other => panic!("replay nonce lama harus ditolak, bukan {other:?}"),
    }

    // Nonce terkini tetap sah: pemulihan kedua menaikkan nonce ke dua.
    let final_release = height + test_policy().unjail_cooldown_blocks;
    let fresh = UnjailRequest::issue(account_id, &master, final_release, 1);
    lifecycle
        .request_unjail(&fresh, final_release)
        .expect("unjail kedua");
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Eligible
    );
    assert_eq!(
        lifecycle.record(&account_id).expect("rekaman").unjail_nonce,
        2
    );
}

#[test]
fn v3_suspension_obeys_matrix_and_uses_longer_cooldown() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (master, account) = onboard(&mut lifecycle, 1, MIN_VALIDATOR_STAKE_QUANTA, 0);
    let account_id = account.account_id;
    assert_eq!(rotate(&mut lifecycle, EPOCH_BLOCKS), vec![account_id]);

    // Penangguhan dewan: status, alasan, dan ketinggian mulai tercatat.
    let suspended_at = EPOCH_BLOCKS + 5;
    lifecycle
        .suspend(
            &account_id,
            SuspensionReason::GuardCouncilVerdict,
            suspended_at,
        )
        .expect("penangguhan sah");
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Suspended
    );
    let suspended = lifecycle.record(&account_id).expect("rekaman");
    assert_eq!(
        suspended.suspension_reason,
        Some(SuspensionReason::GuardCouncilVerdict)
    );
    assert_eq!(suspended.jail_start_block, suspended_at);
    assert_eq!(suspended.active_since_block, 0);
    assert_eq!(lifecycle.active_validator_count(), 0);
    assert!(lifecycle.active_set().is_empty());

    // Matriks: validator masih Probation tidak boleh ditangguhkan.
    let (_m2, probation) = candidate_account(2, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &probation, &key(102), 0).expect("penerimaan kandidat");
    let digest = lifecycle.state_digest();
    match lifecycle.suspend(
        &probation.account_id,
        SuspensionReason::GovernanceVote,
        suspended_at,
    ) {
        Err(ValidatorError::InvalidStatusTransition { from, to }) => {
            assert_eq!(from, ValidatorStatus::Probation);
            assert_eq!(to, ValidatorStatus::Suspended);
        }
        other => panic!("penangguhan probation harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);
    assert_eq!(
        status_of(&lifecycle, &probation.account_id),
        ValidatorStatus::Probation
    );

    // Masa denda tangguh (50 blok) lebih panjang daripada masa unjail.
    let cooldown = test_policy().suspension_cooldown_blocks;
    assert!(cooldown > test_policy().unjail_cooldown_blocks);
    let too_early_at = suspended_at + cooldown - 1;
    match lifecycle.request_unjail(
        &UnjailRequest::issue(account_id, &master, too_early_at, 0),
        too_early_at,
    ) {
        Err(ValidatorError::UnjailCooldownActive { remaining }) => {
            assert_eq!(remaining, 1);
        }
        other => panic!("pemulihan dini harus ditolak, bukan {other:?}"),
    }

    // Pemulihan sah mengembalikan ke Eligible (bukan langsung ActiveSet).
    let released_at = suspended_at + cooldown;
    lifecycle
        .request_unjail(
            &UnjailRequest::issue(account_id, &master, released_at, 0),
            released_at,
        )
        .expect("pemulihan sah");
    assert_eq!(
        status_of(&lifecycle, &account_id),
        ValidatorStatus::Eligible
    );
    let recovered = lifecycle.record(&account_id).expect("rekaman");
    assert_eq!(recovered.suspension_reason, None);
    assert_eq!(recovered.unjail_nonce, 1);
    assert_eq!(lifecycle.active_validator_count(), 0);

    // Kursi hanya kembali melalui rotasi batas epoch berikutnya.
    let boundary = 7 * EPOCH_BLOCKS;
    assert!(boundary > released_at);
    let reactivated = rotate(&mut lifecycle, boundary);
    assert!(reactivated.contains(&account_id));
    assert_eq!(lifecycle.active_validator_count(), 1);
    // Kandidat probation tetap tidak ikut terpilih.
    assert_eq!(
        status_of(&lifecycle, &probation.account_id),
        ValidatorStatus::Probation
    );
}

// ==========================================================================
// V4 — Integritas endorsement: kuota, masa kerja, anti-Sybil, penodaan
// ==========================================================================

#[test]
fn v4_endorsement_enforces_tenure_target_and_quota_gates() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_e1, endorser_1) = onboard(&mut lifecycle, 1, MIN_VALIDATOR_STAKE_QUANTA, 0);
    let (_e2, endorser_2) = onboard(&mut lifecycle, 2, MIN_VALIDATOR_STAKE_QUANTA, 0);
    assert_eq!(rotate(&mut lifecycle, EPOCH_BLOCKS).len(), 2);
    let mut candidates: Vec<SovereignAccount> = Vec::new();
    for seed in 11_u8..=14 {
        let (_master, candidate) = candidate_account(seed, MIN_VALIDATOR_STAKE_QUANTA);
        register_validator(&mut lifecycle, &candidate, &key(seed + 80), 0)
            .expect("penerimaan kandidat");
        candidates.push(candidate);
    }

    // Pengesahan diri selalu ditolak lebih dulu.
    match lifecycle.endorse(
        &endorser_1.account_id,
        &endorser_1.account_id,
        EPOCH_BLOCKS + 20,
    ) {
        Err(ValidatorError::SelfEndorsementForbidden) => {}
        other => panic!("pengesahan diri harus ditolak, bukan {other:?}"),
    }

    // Hanya anggota ActiveSet yang boleh mengesahkan.
    match lifecycle.endorse(
        &candidates[0].account_id,
        &candidates[1].account_id,
        EPOCH_BLOCKS + 20,
    ) {
        Err(ValidatorError::NotActiveInConsensus { status }) => {
            assert_eq!(status, ValidatorStatus::Probation);
        }
        other => panic!("pengesah non-aktif harus ditolak, bukan {other:?}"),
    }

    // Masa kerja minimum terukur dari blok terpilih; satu blok kurang ditolak.
    let min_tenure = test_policy().min_endorser_tenure_blocks;
    let too_young = EPOCH_BLOCKS + min_tenure - 1;
    match lifecycle.endorse(&endorser_1.account_id, &candidates[0].account_id, too_young) {
        Err(ValidatorError::EndorserTenureTooShort { tenure, required }) => {
            assert_eq!(tenure, min_tenure - 1);
            assert_eq!(required, min_tenure);
        }
        other => panic!("masa kerja pendek harus ditolak, bukan {other:?}"),
    }

    // Target harus berstatus Probation/Eligible: sesama anggota aktif ditolak.
    let eligible_height = EPOCH_BLOCKS + min_tenure;
    match lifecycle.endorse(
        &endorser_1.account_id,
        &endorser_2.account_id,
        eligible_height,
    ) {
        Err(ValidatorError::EndorsementTargetNotEligible { status }) => {
            assert_eq!(status, ValidatorStatus::ActiveSet);
        }
        other => panic!("target aktif harus ditolak, bukan {other:?}"),
    }

    // Tiga endorsement sah memenuhi kuota bawaan tanpa menduplikasi bobot.
    for candidate in candidates.iter().take(3) {
        lifecycle
            .endorse(
                &endorser_1.account_id,
                &candidate.account_id,
                eligible_height,
            )
            .expect("endorsement sah");
        assert_eq!(lifecycle.endorsement_weight(&candidate.account_id), 1);
        assert_eq!(lifecycle.probation_requirement(&candidate.account_id), 75);
    }
    assert_eq!(
        lifecycle.endorsement_count(&endorser_1.account_id),
        MAX_ACTIVE_ENDORSEMENTS
    );
    let digest = lifecycle.state_digest();

    // Kuota habis: endorsement keempat ditolak tanpa menyentuh state.
    match lifecycle.endorse(
        &endorser_1.account_id,
        &candidates[3].account_id,
        eligible_height,
    ) {
        Err(ValidatorError::EndorsementQuotaExceeded { active, max }) => {
            assert_eq!(active, MAX_ACTIVE_ENDORSEMENTS);
            assert_eq!(max, MAX_ACTIVE_ENDORSEMENTS);
        }
        other => panic!("kuota terlampaui harus ditolak, bukan {other:?}"),
    }

    // Duplikat dari pengesah yang sama juga tertolak atomik.
    match lifecycle.endorse(
        &endorser_1.account_id,
        &candidates[0].account_id,
        eligible_height,
    ) {
        Err(ValidatorError::DuplicateEndorsement {
            endorser,
            candidate,
        }) => {
            assert_eq!(endorser, endorser_1.account_id);
            assert_eq!(candidate, candidates[0].account_id);
        }
        other => panic!("endorsement duplikat harus ditolak, bukan {other:?}"),
    }

    assert_eq!(lifecycle.state_digest(), digest);
    assert_eq!(lifecycle.endorsement_count(&endorser_1.account_id), 3);
    assert_eq!(lifecycle.endorsement_weight(&candidates[3].account_id), 0);
}

#[test]
fn v4_ledger_enforces_quota_duplicate_taint_and_reciprocal_weight() {
    let mut ledger = EndorsementLedger::new(MAX_ACTIVE_ENDORSEMENTS);
    assert_eq!(ledger.max_active_per_endorser(), MAX_ACTIVE_ENDORSEMENTS);
    let endorser = derive_account_id(&key(21).public_key_bytes());
    let candidate_b = derive_account_id(&key(22).public_key_bytes());
    let candidate_c = derive_account_id(&key(23).public_key_bytes());
    let candidate_d = derive_account_id(&key(24).public_key_bytes());
    let candidate_e = derive_account_id(&key(25).public_key_bytes());

    ledger
        .grant(&endorser, &candidate_b, 1)
        .expect("endorsement pertama");
    assert!(ledger.contains(&endorser, &candidate_b));
    assert_eq!(ledger.active_count(&endorser), 1);
    assert_eq!(ledger.anti_sybil_weight(&candidate_b), 1);
    assert_eq!(ledger.endorsers_of(&candidate_b), vec![endorser]);

    // Pasangan timbal balik saling membatalkan bobot anti-Sybil (bukan 2).
    ledger
        .grant(&candidate_b, &endorser, 2)
        .expect("endorsement balik");
    assert_eq!(ledger.anti_sybil_weight(&candidate_b), 0);
    assert_eq!(ledger.anti_sybil_weight(&endorser), 0);

    match ledger.grant(&endorser, &candidate_b, 3) {
        Err(ValidatorError::DuplicateEndorsement {
            endorser: found_endorser,
            candidate,
        }) => {
            assert_eq!(found_endorser, endorser);
            assert_eq!(candidate, candidate_b);
        }
        other => panic!("duplikat harus ditolak, bukan {other:?}"),
    }

    // Kuota terisi penuh lalu menolak pengesahan berikutnya.
    ledger
        .grant(&endorser, &candidate_c, 3)
        .expect("endorsement kedua");
    ledger
        .grant(&endorser, &candidate_d, 4)
        .expect("endorsement ketiga");
    assert_eq!(ledger.active_count(&endorser), MAX_ACTIVE_ENDORSEMENTS);
    match ledger.grant(&endorser, &candidate_e, 5) {
        Err(ValidatorError::EndorsementQuotaExceeded { active, max }) => {
            assert_eq!(active, MAX_ACTIVE_ENDORSEMENTS);
            assert_eq!(max, MAX_ACTIVE_ENDORSEMENTS);
        }
        other => panic!("kuota terlampaui harus ditolak, bukan {other:?}"),
    }

    // Penodaan mencabut seluruh endorsement dan melarang penerbitan ulang.
    assert_eq!(ledger.anti_sybil_weight(&candidate_c), 1);
    let revoked = ledger.taint_endorser(&endorser);
    assert_eq!(revoked, MAX_ACTIVE_ENDORSEMENTS);
    assert!(ledger.is_tainted(&endorser));
    assert_eq!(ledger.active_count(&endorser), 0);
    assert_eq!(ledger.anti_sybil_weight(&candidate_c), 0);
    assert_eq!(ledger.anti_sybil_weight(&candidate_b), 0);
    match ledger.grant(&endorser, &candidate_c, 6) {
        Err(ValidatorError::EndorserTainted(found)) => assert_eq!(found, endorser),
        other => panic!("pengesah ternodai harus ditolak, bukan {other:?}"),
    }

    // Buku besar kanonikal: urutan penerbitan tidak mengubah digest.
    let actor = derive_account_id(&key(31).public_key_bytes());
    let target_one = derive_account_id(&key(32).public_key_bytes());
    let target_two = derive_account_id(&key(33).public_key_bytes());
    let mut first = EndorsementLedger::new(MAX_ACTIVE_ENDORSEMENTS);
    first.grant(&actor, &target_one, 5).expect("urutan pertama");
    first.grant(&actor, &target_two, 5).expect("urutan pertama");
    let mut second = EndorsementLedger::new(MAX_ACTIVE_ENDORSEMENTS);
    second.grant(&actor, &target_two, 5).expect("urutan kedua");
    second.grant(&actor, &target_one, 5).expect("urutan kedua");
    assert_eq!(first.digest(), second.digest());
}

#[test]
fn v4_endorsements_discount_probation_window_down_to_floor() {
    let policy = ValidatorPolicy {
        max_active_validators: 4,
        ..test_policy()
    };
    let mut lifecycle = ValidatorLifecycle::new(policy).expect("kebijakan valid");
    let mut endorsers: Vec<SovereignAccount> = Vec::new();
    for seed in 1_u8..=4 {
        let (_master, account) = onboard(&mut lifecycle, seed, MIN_VALIDATOR_STAKE_QUANTA, 0);
        endorsers.push(account);
    }
    assert_eq!(rotate(&mut lifecycle, EPOCH_BLOCKS).len(), 4);

    let (_master, candidate) = candidate_account(10, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &candidate, &key(110), 0).expect("penerimaan kandidat");
    let candidate_id = candidate.account_id;

    // Tanpa endorsement: jendela penuh K blok.
    assert_eq!(
        lifecycle.probation_requirement(&candidate_id),
        PROBATION_BLOCKS
    );

    let height = EPOCH_BLOCKS + test_policy().min_endorser_tenure_blocks;
    let floor = PROBATION_BLOCKS / PROBATION_FLOOR_DIVISOR;
    let expected_windows = [(0, 75_u64), (1, 50_u64), (2, 25_u64), (3, 25_u64)];
    for (index, expected) in expected_windows {
        lifecycle
            .endorse(&endorsers[index].account_id, &candidate_id, height)
            .expect("endorsement sah");
        let requirement = lifecycle.probation_requirement(&candidate_id);
        assert_eq!(requirement, expected, "bobot {} diharapkan", index + 1);
        assert!(
            requirement >= floor,
            "jendela tidak boleh di bawah lantai {floor}"
        );
    }

    // Duplikat tidak mempercepat kelulusan maupun mengubah state.
    let digest = lifecycle.state_digest();
    match lifecycle.endorse(&endorsers[0].account_id, &candidate_id, height) {
        Err(ValidatorError::DuplicateEndorsement { .. }) => {}
        other => panic!("duplikat harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);
    assert_eq!(lifecycle.probation_requirement(&candidate_id), floor);

    // Kandidat lulus tepat pada jendela efektif (25 blok tercatat).
    drive_probation(&mut lifecycle, &candidate_id);
    assert_eq!(
        status_of(&lifecycle, &candidate_id),
        ValidatorStatus::Eligible
    );
}

#[test]
fn v4_severe_slashing_taints_endorser_and_revokes_weights() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    // Pengesah berat (stake pas) dan pengesah pembanding (stake lebih besar).
    let (_m1, heavy) = onboard(&mut lifecycle, 1, MIN_VALIDATOR_STAKE_QUANTA, 0);
    let (_m2, control) = onboard(&mut lifecycle, 2, MIN_VALIDATOR_STAKE_QUANTA + 100_000, 0);
    assert_eq!(rotate(&mut lifecycle, EPOCH_BLOCKS).len(), 2);
    let mut candidates: Vec<SovereignAccount> = Vec::new();
    for seed in 11_u8..=13 {
        let (_master, candidate) = candidate_account(seed, MIN_VALIDATOR_STAKE_QUANTA);
        register_validator(&mut lifecycle, &candidate, &key(seed + 80), 0)
            .expect("penerimaan kandidat");
        candidates.push(candidate);
    }

    let height = EPOCH_BLOCKS + test_policy().min_endorser_tenure_blocks;
    for candidate in &candidates {
        lifecycle
            .endorse(&heavy.account_id, &candidate.account_id, height)
            .expect("endorsement pengesah berat");
    }
    lifecycle
        .endorse(&control.account_id, &candidates[0].account_id, height)
        .expect("endorsement pengesah pembanding");
    assert_eq!(lifecycle.endorsement_weight(&candidates[0].account_id), 2);
    assert_eq!(lifecycle.endorsement_weight(&candidates[1].account_id), 1);
    assert_eq!(
        lifecycle.probation_requirement(&candidates[0].account_id),
        50
    );

    // Tarif di luar 10.000 BPS dan akun tak terdaftar ditolak tanpa mutasi.
    let digest = lifecycle.state_digest();
    match lifecycle.slash(&control.account_id, BPS_DENOMINATOR + 1, height) {
        Err(ValidatorError::InvalidSlashRate { rate_bps }) => {
            assert_eq!(rate_bps, BPS_DENOMINATOR + 1);
        }
        other => panic!("tarif tidak sah harus ditolak, bukan {other:?}"),
    }
    let stranger = derive_account_id(&key(60).public_key_bytes());
    match lifecycle.slash(&stranger, SEVERE_SLASH_RATE_BPS, height) {
        Err(ValidatorError::NotRegistered(found)) => assert_eq!(found, stranger),
        other => panic!("akun asing harus ditolak, bukan {other:?}"),
    }
    assert_eq!(lifecycle.state_digest(), digest);

    // Slash lemah (500 BPS): potongan tepat, status dan endorsement utuh.
    let weak = lifecycle
        .slash(&control.account_id, 500, height)
        .expect("slash lemah");
    assert_eq!(weak.slashed_quanta, 55_000); // (MIN+100k) * 500 / 10.000
    assert!(!weak.suspended);
    assert_eq!(weak.endorsements_revoked, 0);
    assert_eq!(
        weak.remaining_stake_quanta,
        MIN_VALIDATOR_STAKE_QUANTA + 45_000
    );
    assert!(!lifecycle.is_endorser_tainted(&control.account_id));
    assert_eq!(
        status_of(&lifecycle, &control.account_id),
        ValidatorStatus::ActiveSet
    );
    assert_eq!(lifecycle.endorsement_weight(&candidates[0].account_id), 2);

    // Slash berat (1.000 BPS): menangguhkan pelaku dan menodai pengesahnya.
    let severe = lifecycle
        .slash(&heavy.account_id, SEVERE_SLASH_RATE_BPS, height)
        .expect("slash berat");
    assert!(severe.suspended);
    assert_eq!(severe.slashed_quanta, 100_000);
    assert_eq!(
        severe.remaining_stake_quanta,
        MIN_VALIDATOR_STAKE_QUANTA - 100_000
    );
    assert_eq!(severe.endorsements_revoked, 3);
    assert_eq!(
        status_of(&lifecycle, &heavy.account_id),
        ValidatorStatus::Suspended
    );
    let suspended = lifecycle.record(&heavy.account_id).expect("rekaman");
    assert_eq!(
        suspended.suspension_reason,
        Some(SuspensionReason::SevereSlashing)
    );
    assert_eq!(suspended.jail_start_block, height);
    assert!(lifecycle.is_endorser_tainted(&heavy.account_id));
    assert!(!lifecycle.is_endorser_tainted(&control.account_id));

    // Pengesahan ternodai dicabut: bobot turun parsial (pengesah pembanding tetap).
    assert_eq!(lifecycle.endorsement_count(&heavy.account_id), 0);
    assert_eq!(lifecycle.endorsement_weight(&candidates[0].account_id), 1);
    assert_eq!(lifecycle.endorsement_weight(&candidates[1].account_id), 0);
    assert_eq!(lifecycle.endorsement_weight(&candidates[2].account_id), 0);
    assert_eq!(
        lifecycle.probation_requirement(&candidates[1].account_id),
        PROBATION_BLOCKS
    );
    assert_eq!(lifecycle.active_validator_count(), 1);
}

// ==========================================================================
// V4 — Tombstone: karantina ireversibel pelaku double-signing
// ==========================================================================

#[test]
fn v4_tombstone_quarantines_probation_double_signer() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(1, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &account, &key(100), 0).expect("penerimaan validator");
    assert_eq!(
        status_of(&lifecycle, &account.account_id),
        ValidatorStatus::Probation
    );
    let stake_before = lifecycle
        .record(&account.account_id)
        .expect("rekaman sah")
        .stake_quanta;

    // Kandidat probation yang terbukti double-signing dapat langsung
    // di-karantina ireversibel tanpa menunggu lulus probation.
    lifecycle
        .tombstone_validator(&account.account_id, TOMBSTONE_EVIDENCE_HEIGHT)
        .expect("karantina probation sah");
    let record = lifecycle.record(&account.account_id).expect("rekaman sah");
    assert_eq!(record.status, ValidatorStatus::Tombstoned);
    assert!(record.status.is_terminal());
    assert!(record.status.is_restricted());
    assert!(record.status.is_tombstoned());
    assert!(!record.status.counts_toward_quorum());
    assert!(!record.status.is_selectable());
    // Karantina mengunci stake tanpa mencairkannya.
    assert_eq!(record.stake_quanta, stake_before);
    assert!(lifecycle.is_endorser_tainted(&account.account_id));
    // Jejak audit: tinggi bukti yang memicu karantina diabadikan pada rekaman.
    assert_eq!(record.tombstone_evidence_height, TOMBSTONE_EVIDENCE_HEIGHT);

    // Tidak ada jalan kembali: tombstone kedua dan pemulihan apapun ditolak.
    assert_eq!(
        lifecycle.tombstone_validator(&account.account_id, TOMBSTONE_EVIDENCE_HEIGHT),
        Err(ValidatorError::InvalidStatusTransition {
            from: ValidatorStatus::Tombstoned,
            to: ValidatorStatus::Tombstoned,
        })
    );
    assert_eq!(lifecycle.active_validator_count(), 0);
}

#[test]
fn v4_tombstone_rejects_statuses_outside_the_matrix() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(2, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &account, &key(100), 0).expect("penerimaan validator");
    drive_probation(&mut lifecycle, &account.account_id);
    assert_eq!(
        status_of(&lifecycle, &account.account_id),
        ValidatorStatus::Eligible
    );

    // Eligible (slot lulus probation, belum di himpunan aktif) berhak langsung
    // di-karantina: double-signer yang lolos masa uji tidak lepas dari tombstone.
    lifecycle
        .tombstone_validator(&account.account_id, TOMBSTONE_EVIDENCE_HEIGHT)
        .expect("karantina slot eligible sah");
    assert_eq!(
        status_of(&lifecycle, &account.account_id),
        ValidatorStatus::Tombstoned
    );

    // `Suspended` menunggu jalur pengampunan tata kelola; karantina langsung
    // dilarang dan state tetap utuh (masih Suspended).
    let (_master2, suspended) = candidate_account(3, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &suspended, &key(101), 0).expect("penerimaan validator");
    drive_probation(&mut lifecycle, &suspended.account_id);
    lifecycle
        .slash(&suspended.account_id, SEVERE_SLASH_RATE_BPS, 1)
        .expect("slash berat sah");
    assert_eq!(
        status_of(&lifecycle, &suspended.account_id),
        ValidatorStatus::Suspended
    );
    assert_eq!(
        lifecycle.tombstone_validator(&suspended.account_id, TOMBSTONE_EVIDENCE_HEIGHT),
        Err(ValidatorError::InvalidStatusTransition {
            from: ValidatorStatus::Suspended,
            to: ValidatorStatus::Tombstoned,
        })
    );
    assert_eq!(
        status_of(&lifecycle, &suspended.account_id),
        ValidatorStatus::Suspended
    );

    // `Retired` (keluar terhormat) juga bukan kandidat tombstone.
    let (_master3, retired) = candidate_account(4, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &retired, &key(102), 0).expect("penerimaan validator");
    drive_probation(&mut lifecycle, &retired.account_id);
    lifecycle
        .retire(&retired.account_id)
        .expect("pensiun terhormat sah");
    assert_eq!(
        lifecycle.tombstone_validator(&retired.account_id, TOMBSTONE_EVIDENCE_HEIGHT),
        Err(ValidatorError::InvalidStatusTransition {
            from: ValidatorStatus::Retired,
            to: ValidatorStatus::Tombstoned,
        })
    );
}

#[test]
fn v4_tombstone_evicts_active_set_and_drops_quorum_count() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let mut accounts = Vec::new();
    for seed in 1_u8..=3 {
        let (_master, account) = candidate_account(seed, MIN_VALIDATOR_STAKE_QUANTA);
        register_validator(&mut lifecycle, &account, &key(seed + 100), 0)
            .expect("penerimaan validator");
        drive_probation(&mut lifecycle, &account.account_id);
        accounts.push(account);
    }
    rotate(&mut lifecycle, EPOCH_BLOCKS);
    assert_eq!(lifecycle.active_validator_count(), 3);

    let target = accounts[0].account_id;
    lifecycle
        .tombstone_validator(&target, TOMBSTONE_EVIDENCE_HEIGHT)
        .expect("karantina anggota himpunan aktif sah");
    assert_eq!(status_of(&lifecycle, &target), ValidatorStatus::Tombstoned);
    assert_eq!(lifecycle.active_validator_count(), 2);
    assert!(!lifecycle.active_set().contains(&target));
}

#[test]
fn v4_tombstone_by_consensus_key_bridges_guard_domain() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(5, MIN_VALIDATOR_STAKE_QUANTA);
    let consensus = key(100);
    register_validator(&mut lifecycle, &account, &consensus, 0).expect("penerimaan validator");

    // Guard menandai pelaku dengan kunci konsensus, bukan AccountId.
    let found = lifecycle
        .tombstone_by_consensus_key(&consensus.public_key_bytes(), TOMBSTONE_EVIDENCE_HEIGHT)
        .expect("karantina via kunci konsensus");
    assert_eq!(found, account.account_id);
    assert!(lifecycle.is_tombstoned_key(&consensus.public_key_bytes()));
    assert!(!lifecycle.is_tombstoned_key(&key(9).public_key_bytes()));
    assert_eq!(
        lifecycle
            .record(&account.account_id)
            .expect("rekaman sah")
            .tombstone_evidence_height,
        TOMBSTONE_EVIDENCE_HEIGHT
    );

    // Kunci konsensus yang tidak dikenal ditolak tanpa mutasi.
    let stranger = key(200);
    assert_eq!(
        lifecycle
            .tombstone_by_consensus_key(&stranger.public_key_bytes(), TOMBSTONE_EVIDENCE_HEIGHT),
        Err(ValidatorError::NotRegistered(stranger.public_key_bytes()))
    );
}

#[test]
fn v4_severe_slash_does_not_resurrect_tombstoned() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_master, account) = candidate_account(6, MIN_VALIDATOR_STAKE_QUANTA);
    register_validator(&mut lifecycle, &account, &key(100), 0).expect("penerimaan validator");
    lifecycle
        .tombstone_validator(&account.account_id, TOMBSTONE_EVIDENCE_HEIGHT)
        .expect("karantina sah");
    let stake_before = lifecycle
        .record(&account.account_id)
        .expect("rekaman sah")
        .stake_quanta;

    // Slash berat setelah tombstone wajib memotong stake namun tidak boleh
    // mengubah status terminal (regresi pra-perbaikan memindahkan ke Retired).
    let outcome = lifecycle
        .slash(&account.account_id, SEVERE_SLASH_RATE_BPS, 1)
        .expect("pemotongan tetap berlaku untuk stake terkunci");
    assert!(outcome.slashed_quanta > 0);
    assert!(outcome.remaining_stake_quanta < stake_before);
    assert_eq!(
        status_of(&lifecycle, &account.account_id),
        ValidatorStatus::Tombstoned
    );
    assert!(lifecycle
        .record(&account.account_id)
        .expect("rekaman sah")
        .status
        .is_terminal());
}

// ==========================================================================
// V5 — Aritmetika BPS nir-pecahan dan pemindaian sumber statis
// ==========================================================================

#[test]
fn v5_bps_arithmetic_is_exact_floor_and_fail_closed() {
    // Uptime: nol data dianggap penuh; pecahan dibulatkan ke bawah.
    assert_eq!(
        uptime_bps(0, 0).expect("tanpa luapan"),
        FULL_PERFORMANCE_BPS
    );
    assert_eq!(uptime_bps(7, 7).expect("tanpa luapan"), BPS_DENOMINATOR);
    assert_eq!(uptime_bps(1, 3).expect("tanpa luapan"), 3_333);
    assert_eq!(uptime_bps(2, 3).expect("tanpa luapan"), 6_666);
    assert_eq!(uptime_bps(0, 1).expect("tanpa luapan"), 0);
    match uptime_bps(4, 3) {
        Err(ValidatorError::InvalidBlockAccounting { signed, eligible }) => {
            assert_eq!(signed, 4);
            assert_eq!(eligible, 3);
        }
        other => panic!("akuntansi blok tak konsisten harus ditolak, bukan {other:?}"),
    }

    // Stake efektif: pembagian bulat ke bawah; luapan menjadi galat bertipe.
    assert_eq!(
        effective_stake_quanta(1_000_000, BPS_DENOMINATOR).expect("tanpa luapan"),
        1_000_000
    );
    assert_eq!(
        effective_stake_quanta(999, 5_000).expect("tanpa luapan"),
        499
    );
    assert_eq!(effective_stake_quanta(1, 9_999).expect("tanpa luapan"), 0);
    match effective_stake_quanta(u64::MAX, BPS_DENOMINATOR) {
        Err(ValidatorError::ArithmeticOverflow) => {}
        other => panic!("luapan stake efektif harus ditolak, bukan {other:?}"),
    }

    // Slash: tarif sah memakai lantai pembagian; tarif >10.000 BPS ditolak.
    assert_eq!(
        slash_amount(1_000_000, 1_000).expect("tanpa luapan"),
        100_000
    );
    assert_eq!(slash_amount(1_999, 5_000).expect("tanpa luapan"), 999);
    assert_eq!(
        slash_amount(999, BPS_DENOMINATOR).expect("tanpa luapan"),
        999
    );
    match slash_amount(10, BPS_DENOMINATOR + 1) {
        Err(ValidatorError::InvalidSlashRate { rate_bps }) => {
            assert_eq!(rate_bps, BPS_DENOMINATOR + 1);
        }
        other => panic!("tarif slash tak sah harus ditolak, bukan {other:?}"),
    }
    match slash_amount(u64::MAX, BPS_DENOMINATOR) {
        Err(ValidatorError::ArithmeticOverflow) => {}
        other => panic!("luapan slash harus ditolak, bukan {other:?}"),
    }

    // Akumulasi bobot: luapan menjadi galat, bukan pembungkusan diam.
    assert_eq!(accumulate_weight(1, 2).expect("tanpa luapan"), 3);
    match accumulate_weight(u64::MAX, 1) {
        Err(ValidatorError::ArithmeticOverflow) => {}
        other => panic!("luapan akumulasi harus ditolak, bukan {other:?}"),
    }

    // Kuorum BPS: perbandingan perkalian murni dan gagal-tertutup.
    assert!(meets_bps_quorum(5_000, 10_000, 5_000));
    assert!(!meets_bps_quorum(4_999, 10_000, 5_000));
    assert!(meets_bps_quorum(1, 3, 3_333));
    assert!(!meets_bps_quorum(1, 3, 3_334));
    assert!(!meets_bps_quorum(100, 0, 5_000));
    assert!(!meets_bps_quorum(100, 500, 0));
    assert!(!meets_bps_quorum(100, 500, BPS_DENOMINATOR + 1));
    assert!(!meets_bps_quorum(u64::MAX, u64::MAX, BPS_DENOMINATOR));
}

#[test]
fn v5_quorum_weight_and_slash_follow_exact_bps_accounting() {
    let mut lifecycle = ValidatorLifecycle::new(test_policy()).expect("kebijakan valid");
    let (_m1, validator_1) = onboard(&mut lifecycle, 1, 1_000_000, 0);
    let (_m2, validator_2) = onboard(&mut lifecycle, 2, 2_000_000, 0);
    let (_m3, validator_3) = onboard(&mut lifecycle, 3, 3_000_000, 0);
    rotate(&mut lifecycle, EPOCH_BLOCKS);
    assert_eq!(lifecycle.active_validator_count(), 3);
    assert_eq!(lifecycle.quorum_weight().expect("tanpa luapan"), 6_000_000);
    assert_eq!(
        lifecycle.total_stake_quanta().expect("tanpa luapan"),
        6_000_000
    );

    // Satu blok terlewat: kinerja 100 * 10.000 / 101 = 9.900 BPS (lantai).
    lifecycle
        .report_block(&validator_1.account_id, EPOCH_BLOCKS + 1, false)
        .expect("blok terlewat");
    let performer = lifecycle.record(&validator_1.account_id).expect("rekaman");
    assert_eq!(performer.performance_bps().expect("tanpa luapan"), 9_900);
    assert_eq!(
        effective_stake_quanta(1_000_000, 9_900).expect("tanpa luapan"),
        990_000
    );
    assert_eq!(lifecycle.quorum_weight().expect("tanpa luapan"), 5_990_000);

    // Slash lemah: potongan bulat ke bawah, keanggotaan aktif tidak berubah.
    let weak = lifecycle
        .slash(&validator_2.account_id, 500, EPOCH_BLOCKS + 2)
        .expect("slash lemah");
    assert_eq!(weak.slashed_quanta, 100_000);
    assert_eq!(weak.remaining_stake_quanta, 1_900_000);
    assert!(!weak.suspended);
    assert_eq!(weak.endorsements_revoked, 0);
    assert_eq!(
        status_of(&lifecycle, &validator_2.account_id),
        ValidatorStatus::ActiveSet
    );
    assert_eq!(lifecycle.quorum_weight().expect("tanpa luapan"), 5_890_000);
    assert_eq!(
        lifecycle.total_stake_quanta().expect("tanpa luapan"),
        5_900_000
    );

    // Slash berat: potongan 10% dan penangguhan mengeluarkan pelaku dari kuorum.
    let severe = lifecycle
        .slash(
            &validator_3.account_id,
            SEVERE_SLASH_RATE_BPS,
            EPOCH_BLOCKS + 3,
        )
        .expect("slash berat");
    assert!(severe.suspended);
    assert_eq!(severe.slashed_quanta, 300_000);
    assert_eq!(severe.remaining_stake_quanta, 2_700_000);
    assert_eq!(
        status_of(&lifecycle, &validator_3.account_id),
        ValidatorStatus::Suspended
    );
    assert_eq!(lifecycle.active_validator_count(), 2);

    // Kuorum BPS membandingkan bobot terhadap total tanpa pembulatan pecahan.
    let weight = lifecycle.quorum_weight().expect("tanpa luapan");
    let total = lifecycle.total_stake_quanta().expect("tanpa luapan");
    assert_eq!(weight, 2_890_000);
    assert_eq!(total, 5_600_000);
    assert!(meets_bps_quorum(weight, total, 5_000));
    assert!(!meets_bps_quorum(weight, total, 5_161));
}

#[test]
fn v5_source_tree_contains_no_floating_point_arithmetic() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut scanned = 0_usize;
    let mut statements = 0_usize;
    for entry in std::fs::read_dir(&source_dir).expect("baca direktori sumber") {
        let entry = entry.expect("entri direktori");
        let path = entry.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("baca berkas sumber");
        scanned += 1;
        for (index, line) in source.lines().enumerate() {
            // Komentar dan doc-comment dikecualikan; hanya kode yang diperiksa.
            let code = line.split("//").next().unwrap_or("").trim();
            if code.is_empty() {
                continue;
            }
            statements += 1;
            let context = format!("{}:{}", path.display(), index + 1);
            // Kandung string dilepas lebih dulu: teks pesan bebas format apa pun.
            let outside_strings: Vec<&str> = code.split('"').step_by(2).collect();
            let scan_area = outside_strings.join(" ");
            assert!(
                !scan_area.contains("f64"),
                "tipe pecahan f64 ditemukan di {context}: {line}"
            );
            assert!(
                !scan_area.contains("f32"),
                "tipe pecahan f32 ditemukan di {context}: {line}"
            );
            // Literal pecahan = digit titik digit; rantai metode `a.b.c()` sah.
            let bytes = scan_area.as_bytes();
            let mut cursor = 0_usize;
            while cursor + 2 < bytes.len() {
                if bytes[cursor].is_ascii_digit()
                    && bytes[cursor + 1] == b'.'
                    && bytes[cursor + 2].is_ascii_digit()
                {
                    panic!("literal pecahan ditemukan di {context}: {line}");
                }
                cursor += 1;
            }
        }
    }
    assert!(
        scanned >= 10,
        "minimal 10 berkas sumber harus dipindai, terbaca {scanned}"
    );
    assert!(
        statements > 100,
        "pemindaian tidak masuk akal: {statements} baris kode"
    );
}
