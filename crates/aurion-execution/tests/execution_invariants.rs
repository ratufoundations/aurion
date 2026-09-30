#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use aurion_execution::{
    compute_state_root, AccountKeeper, Action, ActionBatch, ArbitraryModule,
    CapabilityHandle, ExecutionContext, ExecutionEngine, ExecutionError,
    ExecutionPolicy, Keeper, NamespaceStore, StakingKeeper, StoreKey, TransactionalCache,
};

// ==============================================================================
// [E0] ISOLASI NAMESPACE & BATAS PARTISI STOREKEY (StoreKey Namespace Isolation)
// ==============================================================================

#[test]
fn test_e0_store_key_namespace_isolation_and_anti_collision() {
    let key_acc = StoreKey::new("acc").expect("StoreKey 'acc' gagal dibuat");
    let key_account = StoreKey::new("account").expect("StoreKey 'account' gagal dibuat");
    let key_gov = StoreKey::new("governance").expect("StoreKey 'gov' gagal dibuat");

    // Pastikan digest namespace unik byte-per-byte
    assert_ne!(key_acc.namespace_digest(), key_account.namespace_digest());
    assert_ne!(key_acc.namespace_digest(), key_gov.namespace_digest());

    // Uji anti-collision: Kualifikasi user_key yang identik pada namespace yang mirip
    let user_key = b"balance:alice";
    let q_acc = key_acc.qualify(user_key);
    let q_account = key_account.qualify(user_key);
    assert_ne!(q_acc, q_account, "Prefix-free guarantee dilanggar!");

    // Verifikasi kepemilikan partisi
    assert!(key_acc.owns(&q_acc));
    assert!(!key_acc.owns(&q_account));
    assert!(key_account.owns(&q_account));

    // Uji penolakan akses lintas partisi pada NamespaceStore
    let mut cache = TransactionalCache::new(std::collections::BTreeMap::new(), 10_000);
    let mut acc_store = NamespaceStore::new(key_acc.clone(), &mut cache);

    // Menulis ke partisi sendiri -> Ok
    assert!(acc_store.set(user_key, b"1000").is_ok());

    // Menulis paksa dengan raw key milik partisi 'account' -> UnauthorizedStoreAccess
    let unauthorized_write = acc_store.write_raw(&q_account, b"9999");
    assert!(matches!(
        unauthorized_write,
        Err(ExecutionError::UnauthorizedStoreAccess { .. })
    ));
}

// ==============================================================================
// [E1] ATOMISITAS TRANSAKSIONAL & ROLLBACK (Transactional Cache Revert Safety)
// ==============================================================================

#[test]
fn test_e1_transactional_cache_rollback_on_failure() {
    let mut engine =
        ExecutionEngine::new(ExecutionPolicy::default(), [0xAA; 32]).expect("Engine gagal dibuat");

    let alice = [1u8; 32];
    let bob = [2u8; 32];
    let dead_addr = [0xEE; 32]; // Alamat terlarang

    // Saldo awal: Alice = 1.000, Bob = 500
    engine.set_balance_genesis(&alice, 1_000).expect("should succeed");
    engine.set_balance_genesis(&bob, 500).expect("should succeed");

    let ctx = ExecutionContext {
        block_height: 1,
        timestamp: 1_000_000,
    };

    // Batch cacat: (1) Alice kirim 200 ke Bob, (2) Alice kirim 100 ke dead_addr (terlarang)
    let mut bad_batch = ActionBatch::new();
    bad_batch.push(Action::Transfer {
        from: alice,
        to: bob,
        amount: 200,
    });
    bad_batch.push(Action::Transfer {
        from: alice,
        to: dead_addr,
        amount: 100,
    });

    let res = engine.execute(&ctx, &bad_batch);
    assert!(
        matches!(res, Err(ExecutionError::ExecutionReverted { .. })),
        "Transaksi dengan aksi cacat wajib di-revert!"
    );

    // Invarian Kritis: Mutasi parsial aksi 1 (Alice -> Bob 200) WAJIB ter-rollback sepenuhnya
    assert_eq!(engine.query_balance(&alice).expect("should succeed"), 1_000);
    assert_eq!(engine.query_balance(&bob).expect("should succeed"), 500);

    // Batch sukses: Alice kirim 200 ke Bob
    let mut good_batch = ActionBatch::new();
    good_batch.push(Action::Transfer {
        from: alice,
        to: bob,
        amount: 200,
    });

    let res_good = engine.execute(&ctx, &good_batch);
    assert!(res_good.is_ok());

    // Saldo setelah commit berhasil
    assert_eq!(engine.query_balance(&alice).expect("should succeed"), 800);
    assert_eq!(engine.query_balance(&bob).expect("should succeed"), 700);
}

// ==============================================================================
// [E2] OTORISASI KAPABILITAS ANTAR-KEEPER (Inter-Keeper Capability Authorization)
// ==============================================================================

#[test]
fn test_e2_inter_keeper_capability_authorization() {
    let mut engine =
        ExecutionEngine::new(ExecutionPolicy::default(), [0xBB; 32]).expect("Engine gagal dibuat");

    let alice = [1u8; 32];
    engine.set_balance_genesis(&alice, 5_000).expect("should succeed");

    let account_keeper = AccountKeeper::new().expect("should succeed");
    let staking_keeper = StakingKeeper::new().expect("should succeed");
    let arbitrary_module = ArbitraryModule::new().expect("should succeed");

    let cap_lock = StakingKeeper::lock_capability();
    let staking_id = staking_keeper.module_id();
    let arbitrary_id = arbitrary_module.module_id();

    // 1. Daftarkan kapabilitas LockBalance hanya untuk StakingKeeper (berlaku s/d blok 100)
    engine
        .registry_mut()
        .grant(cap_lock.clone(), staking_id, 100)
        .expect("should succeed");

    let valid_handle = engine
        .registry()
        .issue_handle(&cap_lock, &staking_id)
        .expect("should succeed");

    let mut cache = TransactionalCache::new(engine.committed_snapshot(), 10_000);

    // 2. Pemanggilan sah oleh StakingKeeper -> Sukses
    {
        let mut acc_store = NamespaceStore::new(account_keeper.store_key().clone(), &mut cache);
        let res = account_keeper.lock_balance(
            &mut acc_store,
            engine.registry(),
            &valid_handle,
            10, // Blok aktif
            &alice,
            1_000,
        );
        assert!(res.is_ok(), "Pemanggilan berkapabilitas sah wajib diterima");
        assert_eq!(account_keeper.balance(&mut acc_store, &alice).expect("should succeed"), 4_000);
    }

    // 3. Modul liar memalsukan handle tanpa grant -> Ditolak CapabilityMissing
    {
        let forged_handle = CapabilityHandle::forged_for_test(cap_lock.clone(), arbitrary_id);
        let mut acc_store = NamespaceStore::new(account_keeper.store_key().clone(), &mut cache);
        let res_forged = account_keeper.lock_balance(
            &mut acc_store,
            engine.registry(),
            &forged_handle,
            10,
            &alice,
            500,
        );
        assert!(matches!(
            res_forged,
            Err(ExecutionError::CapabilityMissing { .. })
        ));
    }

    // 4. Penggunaan handle sah setelah masa kedaluwarsa (blok 101 > 100) -> Ditolak CapabilityExpired
    {
        let mut acc_store = NamespaceStore::new(account_keeper.store_key().clone(), &mut cache);
        let res_expired = account_keeper.lock_balance(
            &mut acc_store,
            engine.registry(),
            &valid_handle,
            101, // Melebihi blok kedaluwarsa
            &alice,
            500,
        );
        assert!(matches!(
            res_expired,
            Err(ExecutionError::CapabilityExpired { .. })
        ));
    }
}

// ==============================================================================
// [E3] KOMPOSABILITAS MULTI-AKSI & REVERSIBILITAS (Compositional Reversibility)
// ==============================================================================

#[test]
fn test_e3_multi_action_compositional_reversibility() {
    let mut engine =
        ExecutionEngine::new(ExecutionPolicy::default(), [0xCC; 32]).expect("Engine gagal dibuat");

    let alice = [1u8; 32];
    let bob = [2u8; 32];
    engine.set_balance_genesis(&alice, 10_000).expect("should succeed");

    let ctx = ExecutionContext {
        block_height: 5,
        timestamp: 1_200_000,
    };

    // Rantai 3 aksi majemuk:
    // A1: Alice transfer 1.000 ke Bob (valid)
    // A2: Bob perbarui metadata nama (valid)
    // A3: Pemicu kegagalan eksplisit (invalid)
    let mut batch = ActionBatch::new();
    batch.push(Action::Transfer {
        from: alice,
        to: bob,
        amount: 1_000,
    });
    batch.push(Action::UpdateMetadata {
        account: bob,
        key: "alias".to_string(),
        value: "robert".to_string(),
    });
    batch.push(Action::FailExplicitly {
        reason: "Simulasi kegagalan komputasi pada langkah ketiga".to_string(),
    });

    let res = engine.execute(&ctx, &batch);
    assert!(res.is_err(), "Batch majemuk wajib gagal jika satu aksi gagal");

    // Verifikasi pemulihan total:
    // Saldo Alice tetap 10.000, saldo Bob tetap 0, dan metadata Bob tidak pernah terbit
    assert_eq!(engine.query_balance(&alice).expect("should succeed"), 10_000);
    assert_eq!(engine.query_balance(&bob).expect("should succeed"), 0);
    assert_eq!(engine.query_metadata(&bob, "alias").expect("should succeed"), None);
}

// ==============================================================================
// [E4] METERING FUEL DETERMINISTIK NIR-PECAHAN (Zero-Float Fuel Metering)
// ==============================================================================

#[test]
fn test_e4_zero_float_fuel_metering_exhaustion() {
    // Alokasikan batas fuel ketat: 80 fuel (kurang untuk 2 aksi transfer yang membutuhkan ~140 fuel)
    let strict_policy = ExecutionPolicy {
        fuel_limit: 80,
        max_actions_per_tx: 5,
    };
    let mut engine =
        ExecutionEngine::new(strict_policy, [0xDD; 32]).expect("Engine gagal dibuat");

    let alice = [1u8; 32];
    let bob = [2u8; 32];
    engine.set_balance_genesis(&alice, 1_000).expect("should succeed");

    let ctx = ExecutionContext {
        block_height: 1,
        timestamp: 1_000_000,
    };

    let mut batch = ActionBatch::new();
    batch.push(Action::Transfer {
        from: alice,
        to: bob,
        amount: 100,
    });
    batch.push(Action::Transfer {
        from: alice,
        to: bob,
        amount: 200,
    });

    let res = engine.execute(&ctx, &batch);
    assert!(
        matches!(res, Err(ExecutionError::OutOfFuel { .. })),
        "Eksekusi wajib berhenti seketika dengan OutOfFuel saat bahan bakar habis"
    );

    // Pastikan mutasi sebelum kehabisan fuel tidak bocor
    assert_eq!(engine.query_balance(&alice).expect("should succeed"), 1_000);
    assert_eq!(engine.query_balance(&bob).expect("should succeed"), 0);

    // Audit Statis Bebas Tipe Pecahan (Zero-Float Guarantee)
    let src_files = [
        include_str!("../src/error.rs"),
        include_str!("../src/store_key.rs"),
        include_str!("../src/fuel.rs"),
        include_str!("../src/cache.rs"),
        include_str!("../src/capability.rs"),
        include_str!("../src/keeper.rs"),
        include_str!("../src/action.rs"),
        include_str!("../src/envelope.rs"),
        include_str!("../src/state_root.rs"),
        include_str!("../src/engine.rs"),
    ];

    for (idx, content) in src_files.iter().enumerate() {
        assert!(
            !content.contains("f32") && !content.contains("f64"),
            "Pelanggaran Zero-Float: Ditemukan tipe float pada berkas index {idx}"
        );
    }
}

// ==============================================================================
// [E5] DETERMINISME STATE DELTA & REPLAY (Deterministic State Delta & Replay)
// ==============================================================================

#[test]
fn test_e5_state_delta_determinism_and_replay_invariance() {
    let policy = ExecutionPolicy {
        fuel_limit: 100_000,
        max_actions_per_tx: 100, // Increased to handle 50 actions
    };
    let salt = [0xEE; 32];

    let mut engine_a = ExecutionEngine::new(policy.clone(), salt).expect("should succeed");
    let mut engine_b = ExecutionEngine::new(policy, salt).expect("should succeed");

    let alice = [1u8; 32];
    let bob = [2u8; 32];
    let charlie = [3u8; 32];

    // Setup state awal identik pada kedua instance
    engine_a.set_balance_genesis(&alice, 100_000).expect("should succeed");
    engine_b.set_balance_genesis(&alice, 100_000).expect("should succeed");

    let initial_snapshot = engine_a.committed_snapshot();

    // Buat 50 aksi deterministik
    let mut batch = ActionBatch::new();
    for i in 1..=25 {
        batch.push(Action::Transfer {
            from: alice,
            to: bob,
            amount: i * 10,
        });
        batch.push(Action::Transfer {
            from: alice,
            to: charlie,
            amount: i * 5,
        });
    }

    let ctx = ExecutionContext {
        block_height: 10,
        timestamp: 2_000_000,
    };

    let outcome_a = engine_a.execute(&ctx, &batch).expect("engine_a execution should succeed");
    let outcome_b = engine_b.execute(&ctx, &batch).expect("engine_b execution should succeed");

    // 1. Determinisme Antar-Instance: Write-set digest dan State root wajib identik 100%
    assert_eq!(outcome_a.delta.digest(), outcome_b.delta.digest());
    assert_eq!(outcome_a.state_root, outcome_b.state_root);
    assert_eq!(outcome_a.fuel_consumed, outcome_b.fuel_consumed);

    // 2. Replay Invariance: Menerapkan delta pada initial snapshot menghasilkan root yang identik
    let mut replayed_state = initial_snapshot;
    outcome_a.delta.apply(&mut replayed_state);
    let replayed_root = compute_state_root(&replayed_state);

    assert_eq!(
        replayed_root, outcome_a.state_root,
        "Replay state root wajib identik dengan state root hasil eksekusi langsung!"
    );
}
