#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Invarian mesin arsip ZIP & pruner: AR0–AR5.

use aurion_core::{Account, Block, BlockHeader, State};
use aurion_criptografi::Keypair;
use aurion_ledger::archive::packer;
use aurion_ledger::archive::pruner;
use aurion_ledger::archive::zipper;
use aurion_ledger::{EpochManifest, LedgerError, LedgerStore};
use std::error::Error;
use tempfile::TempDir;

const SAFETY_HORIZON: u64 = 100;

fn alice_kp() -> Keypair {
    Keypair::from_bytes(&[0x01; 32])
}

fn bob_kp() -> Keypair {
    Keypair::from_bytes(&[0x02; 32])
}

fn make_block(height: u64, prev_hash: [u8; 32], state: &State, timestamp: u64) -> Block {
    Block {
        header: BlockHeader {
            height,
            prev_hash,
            state_root: state.compute_state_root(),
            tx_count: 0,
            timestamp,
            proposer: [0; 32],
        },
        transactions: Vec::new(),
    }
}

fn funded_store(
    dir: &TempDir,
    alice: &Keypair,
    bob: &Keypair,
) -> Result<LedgerStore, Box<dyn Error>> {
    let store = LedgerStore::open(dir.path().join("ledger.redb"))?;
    let mut state = State::new();
    state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
    state.insert_account(bob.public_key_bytes(), Account::new(500_000, 0));
    let genesis = make_block(0, [0; 32], &state, 0);
    store.commit_block(&genesis, &mut state)?;
    Ok(store)
}

fn append_blocks(store: &LedgerStore, state: &mut State, count: u64) -> Result<(), Box<dyn Error>> {
    let mut prev_hash = {
        let genesis = store.get_block_by_height(0)?.ok_or("genesis not found")?;
        genesis.header.hash()
    };
    for h in 1..=count {
        let block = make_block(h, prev_hash, state, h * 1_000);
        store.commit_block(&block, state)?;
        prev_hash = block.header.hash();
    }
    Ok(())
}

/// AR0: pruner menolak memotong blok jika horizon tidak terpenuhi.
#[test]
fn test_ar0_safety_horizon_rejection() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let alice = alice_kp();
    let bob = bob_kp();
    let store = funded_store(&dir, &alice, &bob)?;
    let mut state = State::new();
    state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
    state.insert_account(bob.public_key_bytes(), Account::new(500_000, 0));
    append_blocks(&store, &mut state, 10)?;

    let db = store.db();
    let result = pruner::prune_range(db, 0, 5, SAFETY_HORIZON);
    assert!(matches!(result, Err(LedgerError::PruneTooEarly { .. })));
    Ok(())
}

/// AR1 + AR2: pack range ke ZIP, verifikasi integritas biner dan checksum.
#[test]
fn test_ar1_ar2_pack_and_verify() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let alice = alice_kp();
    let bob = bob_kp();
    let store = funded_store(&dir, &alice, &bob)?;
    let mut state = State::new();
    state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
    state.insert_account(bob.public_key_bytes(), Account::new(500_000, 0));
    append_blocks(&store, &mut state, 10)?;

    let db = store.db();
    let packed = packer::pack_range(db, 0, 0, 5, 1_700_000_000)?;

    assert_eq!(packed.manifest.epoch_index, 0);
    assert_eq!(packed.manifest.start_height, 0);
    assert_eq!(packed.manifest.end_height, 5);
    assert_eq!(packed.manifest.block_count, 6);

    let archive_path = dir.path().join("aurion-epoch-0.zip");
    zipper::write_archive(&archive_path, &packed)?;

    let (manifest, blocks_bin, checksum) = zipper::read_archive(&archive_path)?;
    assert_eq!(manifest, packed.manifest);
    assert_eq!(blocks_bin, packed.blocks_bin);
    assert_eq!(checksum, packed.checksum);

    let blocks = packer::unpack_blocks(&blocks_bin)?;
    assert_eq!(blocks.len(), 6);
    assert_eq!(blocks[0].header.height, 0);
    assert_eq!(blocks[5].header.height, 5);
    Ok(())
}

/// AR2: manipulasi 1 bit pada checksum memicu kegagalan validasi.
#[test]
fn test_ar2_checksum_tamper_detection() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let alice = alice_kp();
    let bob = bob_kp();
    let store = funded_store(&dir, &alice, &bob)?;
    let mut state = State::new();
    state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
    state.insert_account(bob.public_key_bytes(), Account::new(500_000, 0));
    append_blocks(&store, &mut state, 10)?;

    let db = store.db();
    let mut packed = packer::pack_range(db, 0, 0, 5, 1_700_000_000)?;
    packed.checksum[0] ^= 0x01;

    let result = packer::verify_checksum(&packed.blocks_bin, &packed.checksum);
    assert!(matches!(result, Err(LedgerError::ChecksumMismatch)));
    Ok(())
}

/// AR3 + AR4: prune atomik, hot state (accounts) tidak berubah.
#[test]
fn test_ar3_ar4_atomic_prune_hot_state_invariance() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let alice = alice_kp();
    let bob = bob_kp();
    let store = funded_store(&dir, &alice, &bob)?;
    let mut state = State::new();
    state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
    state.insert_account(bob.public_key_bytes(), Account::new(500_000, 0));
    append_blocks(&store, &mut state, 200)?;

    let alice_before = store
        .get_account(&alice.public_key_bytes())?
        .ok_or("alice not found")?;
    let bob_before = store
        .get_account(&bob.public_key_bytes())?
        .ok_or("bob not found")?;

    let db = store.db();
    pruner::prune_range(db, 0, 50, SAFETY_HORIZON)?;

    let alice_after = store
        .get_account(&alice.public_key_bytes())?
        .ok_or("alice not found")?;
    let bob_after = store
        .get_account(&bob.public_key_bytes())?
        .ok_or("bob not found")?;

    assert_eq!(alice_before.balance, alice_after.balance);
    assert_eq!(alice_before.nonce, alice_after.nonce);
    assert_eq!(bob_before.balance, bob_after.balance);
    assert_eq!(bob_before.nonce, bob_after.nonce);

    assert!(store.get_block_by_height(0)?.is_none());
    assert!(store.get_block_by_height(50)?.is_none());
    assert!(store.get_block_by_height(51)?.is_some());
    Ok(())
}

/// AR5: manifest menolak nilai float dalam deserialisasi.
#[test]
fn test_ar5_zero_float_manifest_conformance() -> Result<(), Box<dyn Error>> {
    let alice = alice_kp();
    let bob = bob_kp();
    let dir = TempDir::new()?;
    let store = funded_store(&dir, &alice, &bob)?;
    let mut state = State::new();
    state.insert_account(alice.public_key_bytes(), Account::new(1_000_000, 0));
    state.insert_account(bob.public_key_bytes(), Account::new(500_000, 0));
    append_blocks(&store, &mut state, 5)?;

    let db = store.db();
    let packed = packer::pack_range(db, 0, 0, 3, 1_700_000_000)?;
    let json = packed.manifest.to_json()?;

    let manifest: EpochManifest = EpochManifest::from_json(&json)?;
    assert_eq!(manifest.block_count, 4);

    let float_json = r#"{"epoch_index":0.5,"start_height":0,"end_height":3,"block_count":4,"final_state_root":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"timestamp":1700000000}"#;
    assert!(EpochManifest::from_json(float_json).is_err());
    Ok(())
}
