#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
#![allow(clippy::cast_possible_truncation)]

use aurion_projection::{
    ProjectionCursor, AddressIndex, ProjectionMetrics,
    PruningManager, PruningConfig, SnapshotManager,
    ProjectionError, ProjectionRecovery
};
use aurion_core::{
    Account, Block, BlockHeader, State, Transaction
};
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};
use aurion_ledger::LedgerStore;
use std::sync::Arc;


#[test]
fn test_p0_ingestion_idempotency() {
    let mut cursor = ProjectionCursor::new();
    
    // Test sequential ingestion
    assert!(cursor.advance(1, [1u8; 32]).is_ok());
    assert_eq!(cursor.height(), 1);
    
    // Test rejection of non-sequential blocks
    let result = cursor.advance(3, [3u8; 32]);
    assert!(matches!(result, Err(ProjectionError::NonSequentialBlock { expected: 2, got: 3 })));
    
    // Test idempotency (re-applying same block)
    let result = cursor.validate_next_height(1);
    assert!(matches!(result, Err(ProjectionError::BlockAlreadyProjected { height: 1 })));
}

#[test]
fn test_p1_address_indexing() -> Result<(), Box<dyn std::error::Error>> {
    let index = AddressIndex::new();
    let alice = [1u8; 32];
    let bob = [2u8; 32];
    
    // Add 15 transactions
    for i in 1..=15 {
        let tx = Transaction::new(alice, bob, 100, i, 0, [0u8; 64]);
        index.add_transaction(&tx, [(i % 256) as u8; 32], i);
    }
    
    assert_eq!(index.get_transaction_count(&alice), 15);
    assert_eq!(index.get_outbound_count(&alice), 15);
    assert_eq!(index.get_inbound_count(&bob), 15);
    
    // Pagination test (limit 10)
    let page1 = index.get_transaction_history(&alice, 10, 0)?;
    assert_eq!(page1.len(), 10);
    assert_eq!(page1[0].block_height, 15); // Reverse chronological
    
    let page2 = index.get_transaction_history(&alice, 10, 10)?;
    assert_eq!(page2.len(), 5);

    Ok(())
}

#[test]
fn test_p2_snapshot_consistency_and_lag() {
    let manager = SnapshotManager::new();
    manager.update_ledger_height(100);
    manager.update_projection_height(95);
    
    assert_eq!(manager.projection_lag(), 5);
    
    // Test consistency check
    assert!(manager.verify_consistency().is_ok());
    
    manager.update_ledger_height(90);
    manager.update_projection_height(95);
    assert!(manager.verify_consistency().is_err());
}

#[test]
fn test_p3_pruning_and_retention() -> Result<(), Box<dyn std::error::Error>> {
    let config = PruningConfig::with_retention_window(1000)?;
    let manager = PruningManager::new(config);
    
    manager.update_projection_height(1500);
    
    let bound = manager.retention_bound();
    assert_eq!(bound, 500);
    
    // Pruning exactly at current height is fine
    assert!(manager.prune(Some(1500)).is_ok());
    
    // Pruning ahead of projection cursor must be rejected
    assert!(matches!(
        manager.prune(Some(2000)),
        Err(ProjectionError::PruneHeightExceedsCursor { .. })
    ));

    Ok(())
}

#[test]
fn test_p5_zero_float_metrics() {
    let metrics = ProjectionMetrics::new();
    
    for _ in 0..75 {
        metrics.record_cache_hit();
    }
    for _ in 0..25 {
        metrics.record_cache_miss();
    }
    
    // 75% hit rate = 7500 BPS
    assert_eq!(metrics.cache_hit_rate_bps(), 7500);
    
    metrics.record_block_processing(2000);
    metrics.record_block_processing(4000);
    assert_eq!(metrics.avg_block_processing_time_us(), 3000);
}

// ==============================================================================
// [P4] PEMULIHAN KERUSAKAN & REKONSTRUKSI DINGIN DETERMINISTIK
// (Crash Recovery & Cold Rebuild Determinism)
// ==============================================================================

/// Bangun blok yang sah: `state_root` dihitung dari state setelah eksekusi.
fn build_block(
    state: &State,
    height: u64,
    prev_hash: Hash256,
    transactions: Vec<Transaction>,
    proposer: PublicKeyBytes,
) -> Block {
    let mut candidate = state.clone();
    for tx in &transactions {
        candidate
            .apply_transaction_with_proposer(tx, proposer)
            .expect("transaksi harus dapat dieksekusi");
    }

    Block {
        header: BlockHeader {
            height,
            prev_hash,
            state_root: candidate.compute_state_root(),
            tx_count: transactions.len() as u32,
            timestamp: 1_700_000_000 + height,
            proposer,
        },
        transactions,
    }
}

/// Alokasi genesis: memoedalkan akun awal pada state kosong.
fn genesis_alloc() -> State {
    let mut state = State::new();
    state.insert_account(
        Keypair::from_bytes(&[1u8; 32]).public_key_bytes(),
        Account::new(10_000_000, 0),
    );
    state.insert_account(
        Keypair::from_bytes(&[2u8; 32]).public_key_bytes(),
        Account::new(0, 0),
    );
    state
}

/// Tulis rantai `blocks` blok ke ledger dan kembalikan state akhir.
fn seed_chain(store: &LedgerStore, blocks: u64) -> State {
    let alice = Keypair::from_bytes(&[1u8; 32]);
    let bob = Keypair::from_bytes(&[2u8; 32]);
    let proposer = Keypair::from_bytes(&[9u8; 32]).public_key_bytes();

    let mut state = genesis_alloc();

    // Blok genesis: tanpa transaksi, mengunci saldo awal.
    let genesis = build_block(&state, 0, [0u8; 32], Vec::new(), proposer);
    store
        .commit_block(&genesis, &mut state)
        .expect("genesis harus dikomit");

    let mut prev_hash = genesis.header.hash();
    for height in 1..=blocks {
        let nonce = height - 1;
        let unsigned = Transaction::new(
            alice.public_key_bytes(),
            bob.public_key_bytes(),
            100,
            nonce,
            1,
            [0u8; 64],
        );
        let signature = alice.sign(&unsigned.digest());
        let tx = Transaction::new(
            unsigned.sender,
            unsigned.recipient,
            100,
            nonce,
            1,
            signature,
        );

        let block = build_block(&state, height, prev_hash, vec![tx], proposer);
        prev_hash = block.header.hash();
        store
            .commit_block(&block, &mut state)
            .expect("blok harus dikomit");
    }

    state
}

#[test]
fn test_p4_cold_rebuild_is_deterministic_and_crash_recoverable() {
    const BLOCKS: u64 = 50;
    let dir = tempfile::TempDir::new().expect("direktori sementara harus tersedia");
    let path = dir.path().join("chain.redb");

    let chain_state = {
        let store = LedgerStore::open(&path).expect("ledger harus dapat dibuka");
        seed_chain(&store, BLOCKS)
    };
    let chain_root = chain_state.compute_state_root();
    let total_height = BLOCKS + 1;

    let alice = Keypair::from_bytes(&[1u8; 32]).public_key_bytes();
    let bob = Keypair::from_bytes(&[2u8; 32]).public_key_bytes();

    // --- 1. Cold rebuild pada instance A (proyeksi baru lahir) ---
    // redb hanya mengizinkan satu handle terbuka per berkas, sehingga rebuild
    // pertama harus diselesaikan sebelum handle kedua dibuka.
    let snapshot_a = {
        let store = LedgerStore::open(&path).expect("ledger harus dapat dibuka");
        let recovery = ProjectionRecovery::new(Arc::new(store));
        recovery
            .cold_rebuild_with_genesis(0, total_height, genesis_alloc())
            .expect("cold rebuild dari nol harus berhasil")
    };

    // --- 2. Cold rebuild pada instance B, proses independen ---
    let recovery_b =
        ProjectionRecovery::new(Arc::new(LedgerStore::open(&path).expect("ledger kedua")));
    let snapshot_b = recovery_b
        .cold_rebuild_with_genesis(0, total_height, genesis_alloc())
        .expect("cold rebuild kedua harus berhasil");

    // --- 3. Determinisme mutlak antar-instance ---
    ProjectionRecovery::verify_determinism(&snapshot_a, &snapshot_b)
        .expect("dua cold rebuild wajib menghasilkan state identik");

    assert_eq!(snapshot_a.height(), snapshot_b.height());
    assert_eq!(*snapshot_a.state_root(), *snapshot_b.state_root());

    // --- 4. Hasil rebuild identik byte-per-byte dengan state rantai hidup ---
    assert_eq!(
        *snapshot_a.state_root(),
        chain_root,
        "state_root hasil rebuild wajib sama dengan rantai asal"
    );

    // 50 transfer x 100 ke Bob, biaya 1 per transaksi ke proposer.
    let moved = 100 * BLOCKS;
    let fees = BLOCKS;
    assert_eq!(
        snapshot_a.get_balance(&alice),
        Some(10_000_000 - moved - fees),
        "saldo Alice setelah rebuild harus sama dengan rantai asal"
    );
    assert_eq!(
        snapshot_a.get_balance(&bob),
        Some(moved),
        "saldo Bob setelah rebuild harus sama dengan rantai asal"
    );
}

#[test]
fn test_p4_cold_rebuild_detects_unavailable_blocks() {
    let dir = tempfile::TempDir::new().expect("direktori sementara harus tersedia");
    let store = LedgerStore::open(dir.path().join("short.redb")).expect("ledger harus dapat dibuka");
    seed_chain(&store, 5);

    let recovery = ProjectionRecovery::new(Arc::new(store));

    // Meminta rentang di luar tinggi ledger harus gagal, bukan menghasilkan
    // snapshot senyap yang kehilangan riwayat.
    assert!(
        matches!(
            recovery.cold_rebuild_with_genesis(0, 50, genesis_alloc()),
            Err(ProjectionError::BlockUnavailable { height: 6 })
        ),
        "cold rebuild wajib menolak blok yang tidak tersedia"
    );
}

#[test]
fn test_p4_cold_rebuild_rejects_tampered_state_root() {
    let dir = tempfile::TempDir::new().expect("direktori sementara harus tersedia");
    let path = dir.path().join("tampered.redb");
    let store = LedgerStore::open(&path).expect("ledger harus dapat dibuka");
    seed_chain(&store, 3);

    let recovery = ProjectionRecovery::new(Arc::new(store));

    // Genesis yang salah dialokasikan akan terdeteksi pada tinggi 0, karena
    // state_root hasil rekonstruksi tidak cocok dengan header yang dikomit.
    let mut wrong = State::new();
    wrong.insert_account(
        Keypair::from_bytes(&[1u8; 32]).public_key_bytes(),
        Account::new(999, 0),
    );

    assert!(
        matches!(
            recovery.cold_rebuild_with_genesis(0, 4, wrong),
            Err(ProjectionError::StateRootMismatch { height: 0, .. })
        ),
        "cold rebuild wajib menolak alokasi genesis yang tidak cocok"
    );
}
