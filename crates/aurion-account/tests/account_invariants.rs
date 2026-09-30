#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Invariant test suite modul `aurion-account` (A0–A5).
//!
//! Setiap test memverifikasi satu invarian protokol yang mengunci kedaulatan
//! identitas, isolasi peran, keamanan mutasi kebijakan, anti-replay delegasi,
//! pertahanan dust/state bloat, dan akuntansi kuota tanpa floating point.

use aurion_account::{
    derive_account_id, derive_account_id_with_domain, meets_integer_quorum, parse_account_id,
    AccountError, AccountId, AccountLifecycle, AdmissionOutcome, DelegatedApproval,
    DelegationContext, DelegationLedger, DeviceRole, KeyRotationProof, MultiSigApproval,
    MultiSigPolicy, Role, RoleAction, RolePromotion, SovereignAccount, SpendingPolicy,
    ADDRESS_DOMAIN_TAG, DAILY_QUOTA_WINDOW_SECONDS, MIN_ACCOUNT_RESERVE_QUANTA,
    MIN_VALIDATOR_STAKE_QUANTA, QUORUM_THRESHOLD_PERCENT,
};
use aurion_criptografi::{Hash256, Hasher, Keypair, PublicKeyBytes};

const CHAIN_ID: u64 = 1001;

fn key(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

fn address_of(seed: u8) -> AccountId {
    derive_account_id(&key(seed).public_key_bytes())
}

/// Akun berdaulat dengan dana awal di atas reserve minimum.
fn funded_account(seed: u8, funding: u128) -> SovereignAccount {
    let master = key(seed).public_key_bytes();
    SovereignAccount::register(address_of(seed), master, 0, funding).expect("registrasi akun valid")
}

fn link_device(
    account: &mut SovereignAccount,
    master: &Keypair,
    device: &Keypair,
    role: DeviceRole,
) {
    account
        .link_device(
            &master.public_key_bytes(),
            device.public_key_bytes(),
            role,
            0,
            0,
        )
        .expect("penautan perangkat oleh master");
}

fn role_promotion(
    signer: &Keypair,
    account: AccountId,
    new_role: Role,
    stake_quanta: u128,
    nonce: u64,
) -> RolePromotion {
    let mut promotion = RolePromotion {
        account,
        new_role,
        stake_quanta,
        nonce,
        signature: [0_u8; 64],
    };
    promotion.signature = signer.sign(&promotion.digest());
    promotion
}

fn apply_promotion(
    account: &mut SovereignAccount,
    master: &Keypair,
    new_role: Role,
    stake_quanta: u128,
) -> Result<(), AccountError> {
    let promotion = role_promotion(
        master,
        account.account_id,
        new_role,
        stake_quanta,
        account.nonce,
    );
    match new_role {
        Role::ValidatorCandidate => {
            account.promote_to_candidate(&promotion, &master.public_key_bytes())
        }
        Role::ActiveValidator => account.activate_validator(&promotion, &master.public_key_bytes()),
        other => panic!("peran {other:?} tidak dipromosikan melalui jalur akun"),
    }
}

// ============================================================
// A0 — DERIVASI ALAMAT BERDAULAT
// ============================================================

#[test]
fn a0_account_id_derivation_is_deterministic_domain_separated_and_avalanching() {
    let public_key = key(11).public_key_bytes();

    let first = derive_account_id(&public_key);
    let second = derive_account_id(&public_key);
    assert_eq!(first, second, "derivasi alamat wajib deterministik");
    assert_eq!(first.len(), 32, "alamat akun wajib 32 bita");

    // Rekomputasi independen: BLAKE3(tag domain || public key).
    let mut payload = Vec::new();
    payload.extend_from_slice(ADDRESS_DOMAIN_TAG);
    payload.extend_from_slice(&public_key);
    assert_eq!(
        Hasher::digest(&payload),
        first,
        "alamat wajib mengikuti konstruksi domain-separated yang dibekukan"
    );

    // Pemisahan domain kriptografis: tag berbeda wajib memberi alamat berbeda.
    let other_domain = derive_account_id_with_domain(b"AURION_ADDR_CANONICAL_V2", &public_key);
    assert_ne!(
        first, other_domain,
        "tag domain lain tidak boleh bertabrakan"
    );
    let untagged = derive_account_id_with_domain(&[], &public_key);
    assert_ne!(first, untagged, "derivasi tanpa tag wajib berbeda");

    // Kunci berbeda -> alamat berbeda.
    assert_ne!(first, derive_account_id(&key(12).public_key_bytes()));

    // Efek avalanche: satu bit kunci berubah -> sekitar separuh bit alamat berubah.
    let mut flipped = public_key;
    flipped[0] ^= 0x01;
    let avalanche = derive_account_id(&flipped);
    let differing_bits: u32 = first
        .iter()
        .zip(avalanche.iter())
        .map(|(left, right)| (left ^ right).count_ones())
        .sum();
    assert!(
        (80..=176).contains(&differing_bits),
        "distribusi bit alamat tidak seimbang: {differing_bits}"
    );
}

#[test]
fn a0_account_id_parser_is_lossless_and_rejects_malformed_lengths() {
    let account = address_of(3);
    assert_eq!(
        parse_account_id(&account).expect("alamat 32 bita wajib diterima"),
        account,
        "parsing alamat wajib lossless"
    );

    for length in [0_usize, 1, 31, 33, 64, 4096] {
        let bytes = vec![0_u8; length];
        match parse_account_id(&bytes) {
            Err(AccountError::InvalidAccountIdLength { got }) => {
                assert_eq!(got, length, "panjang yang dilaporkan wajib sesuai input");
            }
            other => panic!("panjang {length} seharusnya ditolak, diterima {other:?}"),
        }
    }
}

// ============================================================
// A1 — PEMISAHAN PERAN (RBAC) & SIKLUS HIDUP PERAN
// ============================================================

#[test]
fn a1_role_separation_rejects_unauthorized_privileged_actions() {
    let matrix = [
        (Role::StandardUser, RoleAction::Vote, false),
        (Role::StandardUser, RoleAction::Propose, false),
        (Role::StandardUser, RoleAction::Slash, false),
        (Role::ValidatorCandidate, RoleAction::Vote, false),
        (Role::ValidatorCandidate, RoleAction::Propose, false),
        (Role::ValidatorCandidate, RoleAction::Slash, false),
        (Role::ActiveValidator, RoleAction::Vote, true),
        (Role::ActiveValidator, RoleAction::Propose, true),
        (Role::ActiveValidator, RoleAction::Slash, false),
        (Role::GuardCouncil, RoleAction::Vote, false),
        (Role::GuardCouncil, RoleAction::Propose, false),
        (Role::GuardCouncil, RoleAction::Slash, true),
    ];

    for (role, action, authorized) in matrix {
        let outcome = role.authorize(action);
        assert_eq!(
            outcome.is_ok(),
            authorized,
            "peran {role:?} atas aksi {action:?} tidak sesuai matriks kapabilitas"
        );
        if let Err(error) = outcome {
            match error {
                AccountError::UnauthorizedRole { expected, actual } => {
                    assert_eq!(actual, role, "peran pelaku wajib dilaporkan apa adanya");
                    assert_eq!(expected, action.required_role());
                    assert_ne!(expected, role, "penolakan hanya sah bila peran berbeda");
                }
                other => panic!("error penolakan peran tidak sesuai: {other:?}"),
            }
        }
    }

    assert!(!Role::StandardUser.is_privileged());
    assert!(Role::ValidatorCandidate.is_privileged());
    assert!(Role::ActiveValidator.can_vote() && Role::ActiveValidator.can_propose());
    assert!(
        !Role::ValidatorCandidate.can_vote(),
        "kandidat belum boleh memilih"
    );
    assert!(
        !Role::ActiveValidator.can_slash(),
        "validator biasa tidak boleh slashing"
    );
    assert!(Role::GuardCouncil.can_slash());
}

#[test]
#[allow(clippy::too_many_lines)]
fn a1_promotion_requires_stake_signature_and_master_authority() {
    let master = key(21);
    let master_key = master.public_key_bytes();
    let account_id = derive_account_id(&master_key);
    let mut account = SovereignAccount::new(account_id, master_key, 0);
    assert_eq!(account.role, Role::StandardUser);
    assert_eq!(account.staked_quanta, 0);

    // Stake di bawah ambang -> ditolak walau tanda tangan master sah.
    let under_staked = role_promotion(
        &master,
        account_id,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA - 1,
        0,
    );
    match account.promote_to_candidate(&under_staked, &master_key) {
        Err(AccountError::InsufficientStake { provided, required }) => {
            assert_eq!(provided, MIN_VALIDATOR_STAKE_QUANTA - 1);
            assert_eq!(required, MIN_VALIDATOR_STAKE_QUANTA);
        }
        other => panic!("seharusnya InsufficientStake, diterima {other:?}"),
    }
    assert_eq!(
        account.role,
        Role::StandardUser,
        "state wajib utuh saat ditolak"
    );
    assert_eq!(account.nonce, 0, "nonce wajib tidak naik saat ditolak");

    // Tanda tangan bukan milik master akun -> ditolak.
    let attacker = key(22);
    let forged = role_promotion(
        &attacker,
        account_id,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
        0,
    );
    match account.promote_to_candidate(&forged, &master_key) {
        Err(AccountError::InvalidSignature) => {}
        other => panic!("seharusnya InvalidSignature, diterima {other:?}"),
    }

    // Otorisasi terikat akun lain -> ditolak.
    let foreign = role_promotion(
        &master,
        address_of(23),
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
        0,
    );
    match account.promote_to_candidate(&foreign, &master_key) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("seharusnya SignerMismatch, diterima {other:?}"),
    }

    // Nonce lampau -> ditolak.
    let stale = role_promotion(
        &master,
        account_id,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
        7,
    );
    match account.promote_to_candidate(&stale, &master_key) {
        Err(AccountError::InvalidNonce { expected, got }) => {
            assert_eq!(expected, 0);
            assert_eq!(got, 7);
        }
        other => panic!("seharusnya InvalidNonce, diterima {other:?}"),
    }

    // Lompatan tahap: aktivasi sebelum berstatus kandidat -> ditolak.
    let premature = role_promotion(
        &master,
        account_id,
        Role::ActiveValidator,
        MIN_VALIDATOR_STAKE_QUANTA,
        0,
    );
    match account.activate_validator(&premature, &master_key) {
        Err(AccountError::UnauthorizedRole { expected, actual }) => {
            assert_eq!(expected, Role::ValidatorCandidate);
            assert_eq!(actual, Role::StandardUser);
        }
        other => panic!("seharusnya UnauthorizedRole, diterima {other:?}"),
    }

    // Perangkat non-master tidak boleh mempromosikan meski tanda tangannya sah.
    let daily = key(24);
    link_device(&mut account, &master, &daily, DeviceRole::DailyOperator);
    let by_daily = role_promotion(
        &daily,
        account_id,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
        0,
    );
    match account.promote_to_candidate(&by_daily, &daily.public_key_bytes()) {
        Err(AccountError::MasterPrivilegeRequired) => {}
        other => panic!("seharusnya MasterPrivilegeRequired, diterima {other:?}"),
    }

    // Jalur sah: kandidat lalu validator aktif.
    let promotion = role_promotion(
        &master,
        account_id,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
        0,
    );
    account
        .promote_to_candidate(&promotion, &master_key)
        .expect("promosi kandidat sah");
    assert_eq!(account.role, Role::ValidatorCandidate);
    assert_eq!(account.staked_quanta, MIN_VALIDATOR_STAKE_QUANTA);
    assert_eq!(account.nonce, 1, "promosi wajib menaikkan nonce akun");
    assert!(
        !account.role.can_vote(),
        "kandidat belum berhak memilih (anti eskalasi dini)"
    );

    let activation = role_promotion(
        &master,
        account_id,
        Role::ActiveValidator,
        MIN_VALIDATOR_STAKE_QUANTA,
        account.nonce,
    );
    account
        .activate_validator(&activation, &master_key)
        .expect("aktivasi validator sah");
    assert_eq!(account.role, Role::ActiveValidator);
    assert!(account.role.authorize(RoleAction::Vote).is_ok());
    assert!(account.role.authorize(RoleAction::Propose).is_ok());
    assert!(account.role.authorize(RoleAction::Slash).is_err());

    // Otorisasi promosi yang sama tidak boleh dipakai ulang (anti-replay).
    match account.activate_validator(&activation, &master_key) {
        Err(AccountError::InvalidNonce { expected, got }) => {
            assert_eq!(expected, account.nonce);
            assert_eq!(got, activation.nonce);
        }
        other => panic!("seharusnya InvalidNonce, diterima {other:?}"),
    }
}

#[test]
fn a1_demotion_revokes_privileges_immediately() {
    let master = key(31);
    let master_key = master.public_key_bytes();
    let account_id = derive_account_id(&master_key);
    let mut account = SovereignAccount::new(account_id, master_key, 0);

    apply_promotion(
        &mut account,
        &master,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
    )
    .expect("promosi kandidat");
    apply_promotion(
        &mut account,
        &master,
        Role::ActiveValidator,
        MIN_VALIDATOR_STAKE_QUANTA,
    )
    .expect("aktivasi validator");
    assert_eq!(account.role, Role::ActiveValidator);

    // Hanya master yang boleh mendemosi.
    let daily = key(32);
    link_device(&mut account, &master, &daily, DeviceRole::DailyOperator);
    match account.demote_to_standard_user(&daily.public_key_bytes()) {
        Err(AccountError::MasterPrivilegeRequired) => {}
        other => panic!("seharusnya MasterPrivilegeRequired, diterima {other:?}"),
    }
    assert_eq!(
        account.role,
        Role::ActiveValidator,
        "demosi ilegal wajib ditolak"
    );

    account
        .demote_to_standard_user(&master_key)
        .expect("demosi oleh master");
    assert_eq!(
        account.role,
        Role::StandardUser,
        "demosi wajib berlaku seketika"
    );
    assert_eq!(account.staked_quanta, 0, "stake wajib dilepas saat demosi");
    assert!(account.role.authorize(RoleAction::Vote).is_err());
    assert!(account.role.authorize(RoleAction::Propose).is_err());
    assert!(account.role.authorize(RoleAction::Slash).is_err());

    // Demosi ganda adalah transisi tidak sah.
    match account.demote_to_standard_user(&master_key) {
        Err(AccountError::UnauthorizedRole { expected, actual }) => {
            assert_eq!(expected, Role::ValidatorCandidate);
            assert_eq!(actual, Role::StandardUser);
        }
        other => panic!("seharusnya UnauthorizedRole, diterima {other:?}"),
    }
}

// ============================================================
// A2 — KESELAMATAN MUTASI KEBIJAKAN (ROTASI KUNCI & MULTI-SIG)
// ============================================================

#[test]
fn a2_multisig_threshold_construction_rejects_bricked_shapes() {
    let a = key(41).public_key_bytes();
    let b = key(42).public_key_bytes();
    let c = key(43).public_key_bytes();
    let d = key(44).public_key_bytes();

    let invalid: [(Vec<PublicKeyBytes>, usize); 6] = [
        (Vec::new(), 1),
        (vec![a], 0),
        (vec![a], 2),
        (vec![a, b], 3),
        (vec![a, a], 1),
        (vec![a, b, a], 2),
    ];
    for (signers, threshold) in invalid {
        let expected_signers = signers.len();
        match MultiSigPolicy::new(&signers, threshold) {
            Err(AccountError::InvalidThreshold {
                threshold: reported_threshold,
                signers: reported_signers,
                ..
            }) => {
                assert_eq!(reported_threshold, threshold);
                assert_eq!(reported_signers, expected_signers);
            }
            other => panic!(
                "konfigurasi {threshold} dari {expected_signers} seharusnya ditolak, diterima {other:?}"
            ),
        }
    }

    let policy = MultiSigPolicy::new(&[a, b, c], 2).expect("2 dari 3 adalah konfigurasi sah");
    assert_eq!(policy.threshold(), 2);
    assert_eq!(policy.signer_count(), 3);
    assert!(!policy.has_quorum(1));
    assert!(policy.has_quorum(2) && policy.has_quorum(3));
    assert!(policy.is_signer(&a) && !policy.is_signer(&d));

    // Penggantian penandatangan asing ditolak; penggantian sah tetap kanonikal.
    match policy.replace_signer(&d, a) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("penggantian penandatangan asing seharusnya ditolak, diterima {other:?}"),
    }
    let replaced = policy.replace_signer(&a, d).expect("ganti penandatangan");
    assert_eq!(replaced.signer_count(), 3);
    assert!(!replaced.is_signer(&a) && replaced.is_signer(&d));
    assert_ne!(replaced.digest(), policy.digest());
}

#[test]
fn a2_approval_collection_requires_distinct_signers_meeting_threshold() {
    let signers = [key(51), key(52), key(53)];
    let keys: Vec<PublicKeyBytes> = signers.iter().map(Keypair::public_key_bytes).collect();
    let policy = MultiSigPolicy::new(&keys, 2).expect("2 dari 3");
    let digest = Hasher::digest(b"payload-mutasi-kebijakan");
    let approval_of =
        |signer: &Keypair| MultiSigApproval::new(signer.public_key_bytes(), signer.sign(&digest));

    // Persetujuan sah namun di bawah ambang -> ditolak.
    match policy.verify_approvals(&digest, &[approval_of(&signers[0])]) {
        Err(AccountError::InvalidThreshold {
            approvals,
            threshold,
            signers: count,
        }) => {
            assert_eq!(approvals, 1);
            assert_eq!(threshold, 2);
            assert_eq!(count, 3);
        }
        other => panic!("seharusnya InvalidThreshold, diterima {other:?}"),
    }

    // Dua persetujuan dari kunci berbeda -> kuorum terpenuhi.
    assert_eq!(
        policy
            .verify_approvals(
                &digest,
                &[approval_of(&signers[0]), approval_of(&signers[1])]
            )
            .expect("kuorum 2 dari 3"),
        2
    );

    // Satu kunci yang sama dua kali tidak boleh dihitung ganda.
    match policy.verify_approvals(
        &digest,
        &[approval_of(&signers[0]), approval_of(&signers[0])],
    ) {
        Err(AccountError::InvalidThreshold { approvals, .. }) => assert_eq!(approvals, 1),
        other => panic!("duplikasi penandatangan seharusnya ditolak, diterima {other:?}"),
    }

    // Penandatangan non-anggota ditolak.
    let outsider = key(54);
    match policy.verify_approvals(&digest, &[approval_of(&outsider), approval_of(&signers[1])]) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("seharusnya SignerMismatch, diterima {other:?}"),
    }

    // Digest berbeda -> tanda tangan lama tidak berlaku untuk aksi baru.
    match policy.verify_approvals(
        &Hasher::digest(b"payload-lain"),
        &[approval_of(&signers[0]), approval_of(&signers[1])],
    ) {
        Err(AccountError::InvalidSignature) => {}
        other => panic!("seharusnya InvalidSignature, diterima {other:?}"),
    }
}

#[test]
fn a2_key_rotation_requires_old_key_proof_and_preserves_authority() {
    let master = key(61);
    let master_key = master.public_key_bytes();
    let account_id = derive_account_id(&master_key);
    let mut account = funded_account(61, MIN_ACCOUNT_RESERVE_QUANTA * 4);
    let new_master = key(62);
    let new_key = new_master.public_key_bytes();

    let proof_of = |old: &Keypair, new_key: PublicKeyBytes, account: AccountId, nonce: u64| {
        let mut proof = KeyRotationProof {
            account,
            old_key: old.public_key_bytes(),
            new_key,
            nonce,
            signature: [0_u8; 64],
        };
        proof.signature = old.sign(&proof.digest());
        proof
    };

    // Bukti terikat akun lain -> ditolak.
    match account.rotate_master_key(&proof_of(&master, new_key, address_of(97), 0), 100) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("seharusnya SignerMismatch, diterima {other:?}"),
    }

    // Nonce lampau -> ditolak.
    match account.rotate_master_key(&proof_of(&master, new_key, account_id, 5), 100) {
        Err(AccountError::InvalidNonce { expected, got }) => {
            assert_eq!(expected, 0);
            assert_eq!(got, 5);
        }
        other => panic!("seharusnya InvalidNonce, diterima {other:?}"),
    }

    // Tanda tangan bukan milik kunci lama yang diklaim -> ditolak.
    let attacker = key(63);
    let mut forged = proof_of(&master, new_key, account_id, 0);
    forged.signature = attacker.sign(&forged.digest());
    match account.rotate_master_key(&forged, 100) {
        Err(AccountError::InvalidSignature) => {}
        other => panic!("seharusnya InvalidSignature, diterima {other:?}"),
    }

    // "Rotasi" ke kunci yang sama bukan perubahan otoritas -> ditolak.
    match account.rotate_master_key(&proof_of(&master, master_key, account_id, 0), 100) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("seharusnya SignerMismatch, diterima {other:?}"),
    }

    // State wajib utuh setelah seluruh penolakan.
    assert!(account.devices.contains_key(&master_key));
    assert!(!account.devices.contains_key(&new_key));
    assert!(account.multisig.is_signer(&master_key));
    assert_eq!(account.nonce, 0, "penolakan tidak boleh menaikkan nonce");

    // Rotasi sah: kunci lama dicabut, kunci baru menjadi Master tunggal.
    let devices_before = account.devices.len();
    account
        .rotate_master_key(&proof_of(&master, new_key, account_id, 0), 100)
        .expect("rotasi kunci master sah");
    assert_eq!(
        account.devices.len(),
        devices_before,
        "rotasi wajib mengganti, bukan menambah, perangkat"
    );
    assert!(
        !account.devices.contains_key(&master_key),
        "kunci lama wajib dicabut"
    );
    assert_eq!(
        account.devices.get(&new_key).map(|record| record.role),
        Some(DeviceRole::Master)
    );
    assert!(!account.multisig.is_signer(&master_key));
    assert!(account.multisig.is_signer(&new_key));
    assert_eq!(account.nonce, 1, "rotasi wajib menaikkan nonce akun");

    // Kunci lama kehilangan seluruh otoritas; kunci baru tetap berwenang.
    match account.update_multisig_policy(&master_key, MultiSigPolicy::single(new_key)) {
        Err(AccountError::DeviceNotRegistered(_)) => {}
        other => panic!("kunci lama seharusnya tidak berwenang lagi, diterima {other:?}"),
    }
    let fresh_device = key(64);
    link_device(
        &mut account,
        &new_master,
        &fresh_device,
        DeviceRole::DailyOperator,
    );
    assert_eq!(
        account
            .devices
            .get(&fresh_device.public_key_bytes())
            .map(|record| record.role),
        Some(DeviceRole::DailyOperator)
    );

    // Bukti rotasi lama tidak dapat diputar ulang (nonce sudah maju).
    match account.rotate_master_key(&proof_of(&master, new_key, account_id, 0), 100) {
        Err(AccountError::InvalidNonce { expected, got }) => {
            assert_eq!(expected, 1);
            assert_eq!(got, 0);
        }
        other => panic!("seharusnya InvalidNonce, diterima {other:?}"),
    }
}

#[test]
fn a2_bricked_policy_mutation_is_rejected_without_touching_state() {
    let master = key(71);
    let master_key = master.public_key_bytes();
    let mut account = funded_account(71, MIN_ACCOUNT_RESERVE_QUANTA * 2);
    let extra_master = key(72).public_key_bytes();

    // Perangkat non-master tidak boleh memutasi kebijakan akun.
    let daily = key(73);
    link_device(&mut account, &master, &daily, DeviceRole::DailyOperator);
    match account.update_multisig_policy(
        &daily.public_key_bytes(),
        MultiSigPolicy::single(master_key),
    ) {
        Err(AccountError::MasterPrivilegeRequired) => {}
        other => panic!("seharusnya MasterPrivilegeRequired, diterima {other:?}"),
    }

    // Perluasan kebijakan oleh master (2 dari 2) -> diterima.
    let expanded = MultiSigPolicy::new(&[master_key, extra_master], 2).expect("2 dari 2 sah");
    let digest_before = account.state_digest();
    account
        .update_multisig_policy(&master_key, expanded.clone())
        .expect("perluasan kebijakan oleh master");
    assert_eq!(account.multisig, expanded);
    assert_ne!(account.state_digest(), digest_before);

    // Kebijakan baru yang mencabut otorisasi master aktif -> bricked, ditolak.
    let orphaned = MultiSigPolicy::single(key(74).public_key_bytes());
    let policy_before = account.multisig.clone();
    match account.update_multisig_policy(&master_key, orphaned) {
        Err(AccountError::BrickedStateMutation) => {}
        other => panic!("seharusnya BrickedStateMutation, diterima {other:?}"),
    }
    assert_eq!(
        account.multisig, policy_before,
        "kebijakan wajib utuh setelah mutasi ditolak"
    );

    // Ambang yang tidak dapat dipenuhi tetap tidak dapat dibangun.
    match MultiSigPolicy::new(&[master_key], 2) {
        Err(AccountError::InvalidThreshold { threshold, .. }) => assert_eq!(threshold, 2),
        other => panic!("ambang 2 dari 1 seharusnya ditolak, diterima {other:?}"),
    }
}

// ============================================================
// A3 — ANTI-REPLAY OTORISASI DELEGASI
// ============================================================

#[test]
fn a3_delegation_binding_rejects_cross_chain_account_and_action_replay() {
    let device = key(81);
    let account = address_of(82);
    let action = Hasher::digest(b"aksi-transfer-delegasi");

    let approval_of =
        |chain_id: u64, account: AccountId, signer: &Keypair, nonce: u64, action: Hash256| {
            let mut approval = DelegatedApproval {
                chain_id,
                account,
                signer: signer.public_key_bytes(),
                nonce,
                action_digest: action,
                signature: [0_u8; 64],
            };
            approval.signature = signer.sign(&approval.digest());
            approval
        };
    let valid = approval_of(CHAIN_ID, account, &device, 0, action);
    let context = DelegationContext::new(CHAIN_ID, account, device.public_key_bytes(), 0, action);
    let mut ledger = DelegationLedger::new();

    // Chain ID berbeda (replay lintas rantai) -> ditolak tanpa mutasi.
    let cross_chain = approval_of(CHAIN_ID + 1, account, &device, 0, action);
    match ledger.execute(&cross_chain, &context) {
        Err(AccountError::InvalidChainId { expected, got }) => {
            assert_eq!(expected, CHAIN_ID);
            assert_eq!(got, CHAIN_ID + 1);
        }
        other => panic!("seharusnya InvalidChainId, diterima {other:?}"),
    }
    assert_eq!(ledger.next_nonce(&account), 0, "nonce wajib tidak bergerak");

    // Otorisasi milik akun lain (replay lintas akun) -> ditolak.
    let foreign_account = approval_of(CHAIN_ID, address_of(83), &device, 0, action);
    match ledger.execute(&foreign_account, &context) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("seharusnya SignerMismatch, diterima {other:?}"),
    }

    // Otorisasi milik perangkat lain -> ditolak.
    let other_device = key(84);
    let wrong_signer = approval_of(CHAIN_ID, account, &other_device, 0, action);
    match ledger.execute(&wrong_signer, &context) {
        Err(AccountError::SignerMismatch) => {}
        other => panic!("seharusnya SignerMismatch, diterima {other:?}"),
    }

    // Digest aksi berbeda -> ditolak.
    let other_action = approval_of(CHAIN_ID, account, &device, 0, Hasher::digest(b"aksi-lain"));
    match ledger.execute(&other_action, &context) {
        Err(AccountError::ActionDigestMismatch) => {}
        other => panic!("seharusnya ActionDigestMismatch, diterima {other:?}"),
    }

    // Tanda tangan dipalsukan (kunci lain) -> ditolak.
    let mut tampered = valid.clone();
    tampered.signature = key(85).sign(&tampered.digest());
    match ledger.execute(&tampered, &context) {
        Err(AccountError::InvalidSignature) => {}
        other => panic!("seharusnya InvalidSignature, diterima {other:?}"),
    }

    // Seluruh penolakan wajib tidak meninggalkan jejak state.
    assert_eq!(ledger.next_nonce(&account), 0);
    assert!(!ledger.is_consumed(&account, 0));

    // Otorisasi sah -> dieksekusi dan nonce maju.
    ledger
        .execute(&valid, &context)
        .expect("otorisasi delegasi sah");
    assert!(ledger.is_consumed(&account, 0));
    assert_eq!(ledger.next_nonce(&account), 1);
}

#[test]
fn a3_nonce_advances_atomically_and_replay_is_rejected() {
    let device = key(86);
    let account = address_of(87);
    let action_one = Hasher::digest(b"aksi-1");
    let action_two = Hasher::digest(b"aksi-2");

    let approval_of = |nonce: u64, action: Hash256| {
        let mut approval = DelegatedApproval {
            chain_id: CHAIN_ID,
            account,
            signer: device.public_key_bytes(),
            nonce,
            action_digest: action,
            signature: [0_u8; 64],
        };
        approval.signature = device.sign(&approval.digest());
        approval
    };
    let context_of = |nonce: u64, action: Hash256| {
        DelegationContext::new(CHAIN_ID, account, device.public_key_bytes(), nonce, action)
    };

    let mut ledger = DelegationLedger::new();
    ledger
        .execute(&approval_of(0, action_one), &context_of(0, action_one))
        .expect("nonce 0 dieksekusi");

    // Replay otorisasi identik -> ReplayDetected.
    match ledger.execute(&approval_of(0, action_one), &context_of(0, action_one)) {
        Err(AccountError::ReplayDetected {
            account: reported,
            nonce,
        }) => {
            assert_eq!(reported, account);
            assert_eq!(nonce, 0);
        }
        other => panic!("seharusnya ReplayDetected, diterima {other:?}"),
    }

    // Otorisasi basi dipakai untuk nonce berikutnya -> ditolak.
    match ledger.execute(&approval_of(0, action_one), &context_of(1, action_one)) {
        Err(AccountError::InvalidNonce { expected, got }) => {
            assert_eq!(expected, 1);
            assert_eq!(got, 0);
        }
        other => panic!("seharusnya InvalidNonce, diterima {other:?}"),
    }

    // Loncat nonce -> ditolak.
    match ledger.execute(&approval_of(2, action_two), &context_of(2, action_two)) {
        Err(AccountError::InvalidNonce { expected, got }) => {
            assert_eq!(expected, 1);
            assert_eq!(got, 2);
        }
        other => panic!("seharusnya InvalidNonce, diterima {other:?}"),
    }

    assert_eq!(
        ledger.next_nonce(&account),
        1,
        "setiap penolakan wajib bersifat atomik (nonce tidak bergerak)"
    );
    assert!(!ledger.is_consumed(&account, 1));

    // Nonce berikutnya yang sah tetap dapat dieksekusi.
    ledger
        .execute(&approval_of(1, action_two), &context_of(1, action_two))
        .expect("nonce 1 dieksekusi");
    assert_eq!(ledger.next_nonce(&account), 2);
    assert!(ledger.is_consumed(&account, 1));
    assert!(ledger.is_consumed(&account, 0));
}

// ============================================================
// A4 — PERTAHANAN DUST & STATE BLOAT
// ============================================================

#[test]
fn a4_new_account_requires_reserve_while_dust_to_existing_is_allowed() {
    let master = key(91);
    let master_key = master.public_key_bytes();
    let account_id = derive_account_id(&master_key);
    // Assertion statis: reserve wajib positif (dijamin saat kompilasi).
    const { assert!(MIN_ACCOUNT_RESERVE_QUANTA > 0) };

    // Di bawah reserve -> akun baru tidak boleh lahir.
    match SovereignAccount::register(account_id, master_key, 0, MIN_ACCOUNT_RESERVE_QUANTA - 1) {
        Err(AccountError::BelowDustThreshold { amount, minimum }) => {
            assert_eq!(amount, MIN_ACCOUNT_RESERVE_QUANTA - 1);
            assert_eq!(minimum, MIN_ACCOUNT_RESERVE_QUANTA);
        }
        other => panic!("seharusnya BelowDustThreshold, diterima {other:?}"),
    }

    // Tepat pada reserve -> diterima, dan dana tersimpan utuh tanpa pemotongan.
    let reserved =
        SovereignAccount::register(account_id, master_key, 0, MIN_ACCOUNT_RESERVE_QUANTA)
            .expect("pendanaan tepat pada reserve wajib diterima");
    assert_eq!(reserved.balance, MIN_ACCOUNT_RESERVE_QUANTA);

    // Keputusan admisi transfer: dust ke akun baru ditolak, ke akun lama sah.
    match AccountLifecycle::admit_transfer(false, MIN_ACCOUNT_RESERVE_QUANTA - 1) {
        Err(AccountError::BelowDustThreshold { amount, minimum }) => {
            assert_eq!(amount, MIN_ACCOUNT_RESERVE_QUANTA - 1);
            assert_eq!(minimum, MIN_ACCOUNT_RESERVE_QUANTA);
        }
        other => panic!("seharusnya BelowDustThreshold, diterima {other:?}"),
    }
    match AccountLifecycle::admit_transfer(false, 0) {
        Err(AccountError::BelowDustThreshold { amount: 0, .. }) => {}
        other => panic!("transfer nol ke akun baru seharusnya ditolak, diterima {other:?}"),
    }
    assert_eq!(
        AccountLifecycle::admit_transfer(false, MIN_ACCOUNT_RESERVE_QUANTA).expect("akun baru"),
        AdmissionOutcome::NewAccountReserved
    );
    assert_eq!(
        AccountLifecycle::admit_transfer(true, 1).expect("dust ke akun lama tetap sah"),
        AdmissionOutcome::ExistingAccount
    );
}

#[test]
fn a4_zero_balance_accounts_are_canonical_and_prunable() {
    let master = key(95);
    let master_key = master.public_key_bytes();
    let account_id = derive_account_id(&master_key);
    let mut account = funded_account(95, 10_000);
    let device = key(96);
    link_device(&mut account, &master, &device, DeviceRole::DailyOperator);

    // Akun tanpa aktivitas bukan kandidat pruning (state masih otoritatif).
    let fresh = SovereignAccount::new(address_of(94), key(94).public_key_bytes(), 0);
    assert!(!AccountLifecycle::is_prunable(&fresh));

    // Seluruh saldo keluar -> saldo tepat nol, tanpa residu pecahan.
    account
        .authorize_transfer(&device.public_key_bytes(), 10_000, 0, 0)
        .expect("transfer penuh oleh perangkat harian");
    assert_eq!(account.balance, 0);
    assert_eq!(account.nonce, 1);
    assert!(
        AccountLifecycle::is_prunable(&account),
        "akun nir-saldo tanpa stake adalah kandidat pruning"
    );

    // Representasi kanonikal: digest state deterministik untuk state yang sama.
    let digest = account.state_digest();
    assert_eq!(account.state_digest(), digest);
    assert_eq!(account.clone().state_digest(), digest);

    // Urutan penautan perangkat tidak mengubah digest kanonikal.
    let mut first = SovereignAccount::new(account_id, master_key, 500);
    let mut second = SovereignAccount::new(account_id, master_key, 500);
    let extra_a = key(97).public_key_bytes();
    let extra_b = key(98).public_key_bytes();
    first
        .link_device(&master_key, extra_a, DeviceRole::ReadOnly, 500, 0)
        .expect("tautkan perangkat a");
    first
        .link_device(&master_key, extra_b, DeviceRole::ReadOnly, 500, 0)
        .expect("tautkan perangkat b");
    second
        .link_device(&master_key, extra_b, DeviceRole::ReadOnly, 500, 0)
        .expect("tautkan perangkat b");
    second
        .link_device(&master_key, extra_a, DeviceRole::ReadOnly, 500, 0)
        .expect("tautkan perangkat a");
    assert_eq!(
        first.state_digest(),
        second.state_digest(),
        "digest kanonikal tidak boleh bergantung urutan penautan"
    );

    // Akun nir-saldo yang menahan stake wajib dipertahankan.
    let promotion = role_promotion(
        &master,
        account_id,
        Role::ValidatorCandidate,
        MIN_VALIDATOR_STAKE_QUANTA,
        account.nonce,
    );
    account
        .promote_to_candidate(&promotion, &master_key)
        .expect("promosi kandidat");
    assert_eq!(account.balance, 0);
    assert!(
        !AccountLifecycle::is_prunable(&account),
        "akun ber-stake tidak boleh dipangkas"
    );

    // Satuan Quanta terkecil pun tersimpan eksak (tanpa pembulatan pecahan).
    account.balance = 7;
    assert_eq!(account.balance, 7);
    assert!(!AccountLifecycle::is_prunable(&account));
}

// ============================================================
// A5 — AKUNTANSI KUOTA TANPA FLOATING POINT
// ============================================================

#[test]
fn a5_integer_quorum_math_is_exact_and_overflow_safe() {
    assert!(meets_integer_quorum(67, 100, QUORUM_THRESHOLD_PERCENT));
    assert!(!meets_integer_quorum(66, 100, QUORUM_THRESHOLD_PERCENT));
    assert!(meets_integer_quorum(100, 100, 100));
    assert!(!meets_integer_quorum(99, 100, 100));

    // 2 dari 3 (66,6%) wajib berada DI BAWAH ambang 67% tanpa pembulatan pecahan.
    assert!(!meets_integer_quorum(2, 3, QUORUM_THRESHOLD_PERCENT));
    assert!(meets_integer_quorum(3, 3, QUORUM_THRESHOLD_PERCENT));

    // Ambang tidak bermakna atau luapan -> false (fail-closed), bukan panic.
    assert!(!meets_integer_quorum(1, 0, QUORUM_THRESHOLD_PERCENT));
    assert!(!meets_integer_quorum(1, 1, 0));
    assert!(!meets_integer_quorum(1, 1, 101));
    assert!(!meets_integer_quorum(u64::MAX, u64::MAX, 100));
    assert!(!meets_integer_quorum(u64::MAX, 1, 100));
}

#[test]
fn a5_quota_accounting_uses_checked_integer_arithmetic() {
    let mut policy = SpendingPolicy::default();
    assert_eq!(policy.remaining_quota(), policy.daily_quota);

    // Pemakaian bertahap dalam satu jendela harian.
    for _ in 0..3 {
        policy
            .check_and_update(50_000, 0)
            .expect("pemakaian masih dalam kuota");
    }
    assert_eq!(policy.current_spent, 150_000);
    assert_eq!(policy.remaining_quota(), 50_000);

    // Melebihi batas per transaksi -> error bertipe, pemakaian tidak berubah.
    match policy.check_and_update(50_001, 0) {
        Err(AccountError::ExceedsPerTxLimit { amount, limit }) => {
            assert_eq!(amount, 50_001);
            assert_eq!(limit, 50_000);
        }
        other => panic!("seharusnya ExceedsPerTxLimit, diterima {other:?}"),
    }
    assert_eq!(policy.current_spent, 150_000);

    // Tepat menghabiskan kuota -> sah; satu Quanta berikutnya ditolak.
    policy
        .check_and_update(50_000, 0)
        .expect("kuota tepat habis masih sah");
    assert_eq!(policy.remaining_quota(), 0);
    match policy.check_and_update(1, 0) {
        Err(AccountError::ExceedsDailyQuota { spent, limit }) => {
            assert_eq!(spent, 200_001);
            assert_eq!(limit, 200_000);
        }
        other => panic!("seharusnya ExceedsDailyQuota, diterima {other:?}"),
    }
    assert_eq!(
        policy.current_spent, 200_000,
        "penolakan tidak boleh mengubah pemakaian"
    );

    // Jendela harian di-reset hanya berdasarkan waktu bilangan bulat.
    let mut windowed = SpendingPolicy::default();
    windowed
        .check_and_update(50_000, DAILY_QUOTA_WINDOW_SECONDS - 1)
        .expect("pemakaian awal");
    windowed
        .check_and_update(50_000, DAILY_QUOTA_WINDOW_SECONDS - 1)
        .expect("akumulasi dalam jendela");
    assert_eq!(windowed.current_spent, 100_000);
    windowed
        .check_and_update(50_000, DAILY_QUOTA_WINDOW_SECONDS)
        .expect("reset jendela harian");
    assert_eq!(
        windowed.current_spent, 50_000,
        "kuota di-reset setelah 24 jam penuh"
    );

    // Luapan akumulasi -> ArithmeticOverflow, state tidak berubah.
    let mut overflowing = SpendingPolicy {
        max_per_tx: u128::MAX,
        daily_quota: u128::MAX,
        current_spent: u128::MAX - 1,
        last_reset_time: 0,
    };
    assert_eq!(overflowing.remaining_quota(), 1);
    match overflowing.check_and_update(10, 0) {
        Err(AccountError::ArithmeticOverflow) => {}
        other => panic!("seharusnya ArithmeticOverflow, diterima {other:?}"),
    }
    assert_eq!(
        overflowing.current_spent,
        u128::MAX - 1,
        "state wajib utuh saat akumulasi meluap"
    );
}

#[test]
fn a5_sources_are_free_of_floating_point_tokens() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let banned = [["f", "32"].concat(), ["f", "64"].concat()];
    let mut scanned: usize = 0;

    for entry in std::fs::read_dir(&source_dir).expect("direktori src wajib terbaca") {
        let path = entry.expect("entri direktori wajib terbaca").path();
        match path.extension() {
            Some(extension) if extension == "rs" => {}
            _ => continue,
        }
        let content = std::fs::read_to_string(&path).expect("berkas sumber wajib terbaca");
        for token in &banned {
            assert!(
                !content.contains(token.as_str()),
                "{} memuat token tipe pecahan terlarang",
                path.display()
            );
        }
        scanned = scanned.saturating_add(1);
    }

    assert!(
        scanned >= 10,
        "seluruh berkas sumber aurion-account wajib dipindai, terpindai {scanned}"
    );
}
