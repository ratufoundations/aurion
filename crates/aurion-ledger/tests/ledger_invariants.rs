#![forbid(unsafe_code)]

use aurion_core::{Account, Block, BlockHeader, State, Transaction};
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};
use aurion_ledger::{
    schema::{ACCOUNTS_TABLE, BLOCKS_TABLE, BLOCK_INDEX_TABLE, METADATA_TABLE, MODULE_KV_TABLE},
    Codec, LedgerError, LedgerSnapshot, LedgerStore,
};
use redb::{Database, ReadableTable};
use std::{error::Error, io, sync::mpsc, thread, time::Duration};
use tempfile::TempDir;

fn empty_block(height: u64, prev_hash: Hash256, state_root: Hash256) -> Block {
    Block {
        header: BlockHeader {
            height,
            prev_hash,
            state_root,
            tx_count: 0,
            timestamp: height * 1_000,
            proposer: [0; 32],
        },
        transactions: Vec::new(),
    }
}

fn genesis_block(state: &State) -> Block {
    empty_block(0, [0; 32], state.compute_state_root())
}

fn signed_transfer(
    keypair: &Keypair,
    recipient: PublicKeyBytes,
    amount: u64,
    nonce: u64,
) -> Transaction {
    let unsigned = Transaction::new(
        keypair.public_key_bytes(),
        recipient,
        amount,
        nonce,
        1,
        [0; 64],
    );
    Transaction::new(
        unsigned.sender,
        unsigned.recipient,
        amount,
        nonce,
        1,
        keypair.sign(&unsigned.digest()),
    )
}

fn snapshot_report(
    snapshot: &LedgerSnapshot,
    key: &PublicKeyBytes,
) -> Result<SnapshotReport, LedgerError> {
    let account = snapshot
        .get_account(key)?
        .ok_or(LedgerError::AccountNotFound(*key))?;
    Ok(SnapshotReport {
        height: snapshot.get_latest_height()?,
        balance: account.balance,
        nonce: account.nonce,
        state_root: snapshot.get_latest_state_root()?,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct SnapshotReport {
    height: u64,
    balance: u64,
    nonce: u64,
    state_root: Option<Hash256>,
}

#[test]
fn l0_codec_round_trips_core_values_and_rejects_truncated_or_trailing_bytes(
) -> Result<(), Box<dyn Error>> {
    let account = Account::new(u64::MAX - 4, u64::MAX - 8);
    let encoded_account = Codec::encode_account(&account);
    assert_eq!(Codec::decode_account(&encoded_account), account);

    let transaction = Transaction::new([0x11; 32], [0x22; 32], u64::MAX - 12, 17, 9, [0xa5; 64]);
    let encoded_transaction = Codec::encode_tx(&transaction);
    assert_eq!(Codec::decode_tx(&encoded_transaction)?, transaction);
    assert!(matches!(
        Codec::decode_tx(&encoded_transaction[..encoded_transaction.len() - 1]),
        Err(LedgerError::InvalidTransactionLength { .. })
    ));
    let mut transaction_with_trailing_byte = encoded_transaction.to_vec();
    transaction_with_trailing_byte.push(0xff);
    assert!(matches!(
        Codec::decode_tx(&transaction_with_trailing_byte),
        Err(LedgerError::InvalidTransactionLength { .. })
    ));

    let block = Block {
        header: BlockHeader {
            height: 9,
            prev_hash: [0x33; 32],
            state_root: [0x44; 32],
            tx_count: 1,
            timestamp: 9_000,
            proposer: [0x55; 32],
        },
        transactions: vec![transaction],
    };
    let encoded_block = Codec::encode_block(&block);
    let decoded_block = Codec::decode_block(&encoded_block)?;
    assert_eq!(decoded_block, block);
    assert_eq!(Codec::encode_block(&decoded_block), encoded_block);

    let mut truncated_block = encoded_block.clone();
    truncated_block.pop();
    assert!(matches!(
        Codec::decode_block(&truncated_block),
        Err(LedgerError::CorruptedBlock(9))
    ));
    let mut block_with_trailing_byte = encoded_block;
    block_with_trailing_byte.push(0xff);
    assert!(matches!(
        Codec::decode_block(&block_with_trailing_byte),
        Err(LedgerError::CorruptedBlock(9))
    ));
    Ok(())
}

#[test]
fn l1_dropped_multitable_write_transaction_leaves_disk_unchanged() -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let database_path = directory.path().join("ledger.redb");
    let baseline_key = [0x11; 32];
    let aborted_key = [0x22; 32];
    let baseline_state = {
        let store = LedgerStore::open(&database_path)?;
        let mut state = State::new();
        state.insert_account(baseline_key, Account::new(50_000, 3));
        store.commit_block(&genesis_block(&state), &mut state)?;
        state
    };
    let baseline_root = baseline_state.compute_state_root();

    {
        let database = Database::open(&database_path)?;
        let write_txn = database.begin_write()?;
        {
            let mut accounts = write_txn.open_table(ACCOUNTS_TABLE)?;
            accounts.insert(&aborted_key, &Codec::encode_account(&Account::new(99, 8)))?;

            let mut blocks = write_txn.open_table(BLOCKS_TABLE)?;
            let aborted_block = empty_block(1, [0x55; 32], baseline_root);
            let encoded_block = Codec::encode_block(&aborted_block);
            blocks.insert(1, encoded_block.as_slice())?;

            let mut block_index = write_txn.open_table(BLOCK_INDEX_TABLE)?;
            block_index.insert(&[0x66; 32], 1)?;

            let mut module_kv = write_txn.open_table(MODULE_KV_TABLE)?;
            module_kv.insert(b"aborted-key".as_slice(), b"uncommitted-value".as_slice())?;
            // Simulated mid-write failure: latest metadata is intentionally not written;
            // dropping the write transaction must abort every table mutation above.
        }
        drop(write_txn);
    }

    let reopened = LedgerStore::open(&database_path)?;
    assert_eq!(reopened.get_latest_height()?, 0);
    assert_eq!(reopened.get_latest_state_root()?, Some(baseline_root));
    assert_eq!(
        reopened.get_account(&baseline_key)?,
        baseline_state.get_account(&baseline_key).copied()
    );
    assert_eq!(reopened.get_account(&aborted_key)?, None);
    assert_eq!(reopened.get_block_by_height(1)?, None);
    drop(reopened);

    let database = Database::open(&database_path)?;
    let read_txn = database.begin_read()?;
    let block_index = read_txn.open_table(BLOCK_INDEX_TABLE)?;
    let module_kv = read_txn.open_table(MODULE_KV_TABLE)?;
    assert!(block_index.get(&[0x66; 32])?.is_none());
    assert!(module_kv.get(b"aborted-key".as_slice())?.is_none());
    Ok(())
}

#[test]
fn l2_account_and_module_partitions_isolate_equal_keys_and_ranges() -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let database = Database::create(directory.path().join("partitions.redb"))?;
    let key = [0x01; 32];
    let account_only_key = [0x02; 32];
    let module_only_key = [0x03; 32];
    let account_value = Codec::encode_account(&Account::new(111, 7));
    let module_value = b"module-value";
    let write_txn = database.begin_write()?;
    {
        let mut accounts = write_txn.open_table(ACCOUNTS_TABLE)?;
        accounts.insert(&key, &account_value)?;
        accounts.insert(
            &account_only_key,
            &Codec::encode_account(&Account::new(222, 8)),
        )?;

        let mut module_kv = write_txn.open_table(MODULE_KV_TABLE)?;
        module_kv.insert(key.as_slice(), module_value.as_slice())?;
        module_kv.insert(module_only_key.as_slice(), b"isolated".as_slice())?;

        let mut blocks = write_txn.open_table(BLOCKS_TABLE)?;
        blocks.insert(1, b"block-partition".as_slice())?;
        let mut block_index = write_txn.open_table(BLOCK_INDEX_TABLE)?;
        block_index.insert(&key, 1)?;
        let mut metadata = write_txn.open_table(METADATA_TABLE)?;
        metadata.insert("latest_height", &1_u64.to_le_bytes()[..])?;
    }
    write_txn.commit()?;

    let read_txn = database.begin_read()?;
    let accounts = read_txn.open_table(ACCOUNTS_TABLE)?;
    let module_kv = read_txn.open_table(MODULE_KV_TABLE)?;
    let blocks = read_txn.open_table(BLOCKS_TABLE)?;
    let block_index = read_txn.open_table(BLOCK_INDEX_TABLE)?;
    let metadata = read_txn.open_table(METADATA_TABLE)?;
    assert_eq!(
        accounts.get(&key)?.map(|value| *value.value()),
        Some(account_value)
    );
    assert_eq!(
        module_kv
            .get(key.as_slice())?
            .map(|value| value.value().to_vec()),
        Some(module_value.to_vec())
    );
    assert!(accounts.get(&module_only_key)?.is_none());
    assert!(module_kv.get(account_only_key.as_slice())?.is_none());
    assert_eq!(
        blocks.get(1)?.map(|value| value.value().to_vec()),
        Some(b"block-partition".to_vec())
    );
    assert_eq!(block_index.get(&key)?.map(|value| value.value()), Some(1));
    assert_eq!(
        metadata
            .get("latest_height")?
            .map(|value| value.value().to_vec()),
        Some(1_u64.to_le_bytes().to_vec())
    );

    let account_rows = accounts.iter()?.collect::<Result<Vec<_>, _>>()?;
    let module_rows = module_kv.iter()?.collect::<Result<Vec<_>, _>>()?;
    let block_rows = blocks.iter()?.collect::<Result<Vec<_>, _>>()?;
    let index_rows = block_index.iter()?.collect::<Result<Vec<_>, _>>()?;
    let metadata_rows = metadata.iter()?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(account_rows.len(), 2);
    assert_eq!(module_rows.len(), 2);
    assert_eq!(block_rows.len(), 1);
    assert_eq!(index_rows.len(), 1);
    assert_eq!(metadata_rows.len(), 1);
    assert!(account_rows
        .iter()
        .all(|(row_key, _)| *row_key.value() != module_only_key));
    assert!(module_rows
        .iter()
        .all(|(row_key, _)| row_key.value() != account_only_key));
    Ok(())
}

#[test]
fn l3_commit_requires_genesis_then_gapless_append_and_rejects_overwrite(
) -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let store = LedgerStore::open(directory.path().join("append-only.redb"))?;
    let mut state = State::new();
    let account_key = [0x31; 32];
    state.insert_account(account_key, Account::new(10_000, 0));
    let state_before_genesis = state.clone();
    let premature_block = empty_block(1, [0; 32], state.compute_state_root());
    assert!(matches!(
        store.commit_block(&premature_block, &mut state),
        Err(LedgerError::NonSequentialBlock {
            expected: 0,
            actual: 1
        })
    ));
    assert_eq!(state, state_before_genesis);
    let genesis = genesis_block(&state);
    store.commit_block(&genesis, &mut state)?;
    assert_eq!(store.get_latest_height()?, 0);

    let block_one = empty_block(1, genesis.header.hash(), state.compute_state_root());
    store.commit_block(&block_one, &mut state)?;
    assert_eq!(store.get_latest_height()?, 1);

    let state_before_rejected_writes = state.clone();
    let forked_block = empty_block(2, [0x98; 32], state.compute_state_root());
    assert!(matches!(
        store.commit_block(&forked_block, &mut state),
        Err(LedgerError::PreviousHashMismatch(2))
    ));
    assert_eq!(state, state_before_rejected_writes);

    let skipped_block = empty_block(3, block_one.header.hash(), state.compute_state_root());
    assert!(matches!(
        store.commit_block(&skipped_block, &mut state),
        Err(LedgerError::NonSequentialBlock {
            expected: 2,
            actual: 3
        })
    ));
    assert_eq!(state, state_before_rejected_writes);

    let conflicting_block = empty_block(1, [0x99; 32], state.compute_state_root());
    assert!(matches!(
        store.commit_block(&conflicting_block, &mut state),
        Err(LedgerError::BlockAlreadyExists(1))
    ));
    assert_eq!(state, state_before_rejected_writes);
    assert_eq!(store.get_latest_height()?, 1);
    assert_eq!(store.get_block_by_height(1)?, Some(block_one));
    Ok(())
}

#[test]
fn l4_read_snapshot_survives_concurrent_commit_and_fresh_reads_see_new_state(
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let directory = TempDir::new()?;
    let store = LedgerStore::open(directory.path().join("snapshot.redb"))?;
    let sender = Keypair::from_bytes(&[0x41; 32]);
    let sender_key = sender.public_key_bytes();
    let recipient_key = [0x42; 32];
    let mut state = State::new();
    state.insert_account(sender_key, Account::new(10_000, 0));
    state.insert_account(recipient_key, Account::new(500, 0));

    let mut previous_hash = genesis_block(&state).header.hash();
    store.commit_block(&genesis_block(&state), &mut state)?;
    for height in 1..=5 {
        let block = empty_block(height, previous_hash, state.compute_state_root());
        previous_hash = block.header.hash();
        store.commit_block(&block, &mut state)?;
    }
    let old_root = state.compute_state_root();
    let snapshot = store.read_snapshot()?;
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (resume_sender, resume_receiver) = mpsc::channel();
    let reader = thread::spawn(
        move || -> Result<SnapshotReport, Box<dyn Error + Send + Sync>> {
            let before = snapshot_report(&snapshot, &sender_key)?;
            ready_sender
                .send(())
                .map_err(|error| io::Error::other(error.to_string()))?;
            resume_receiver.recv_timeout(Duration::from_secs(10))?;
            let after = snapshot_report(&snapshot, &sender_key)?;
            assert_eq!(before, after);
            Ok(after)
        },
    );
    ready_receiver.recv_timeout(Duration::from_secs(10))?;

    let writer_store = store.clone();
    let mut writer_state = state.clone();
    let transaction = signed_transfer(&sender, recipient_key, 1_000, 0);
    let mut expected_state = writer_state.clone();
    expected_state.apply_transaction(&transaction)?;
    let block_six = Block {
        header: BlockHeader {
            height: 6,
            prev_hash: previous_hash,
            state_root: expected_state.compute_state_root(),
            tx_count: 1,
            timestamp: 6_000,
            proposer: [0; 32],
        },
        transactions: vec![transaction],
    };
    let (write_done_sender, write_done_receiver) = mpsc::channel();
    let writer = thread::spawn(move || {
        let result = writer_store.commit_block(&block_six, &mut writer_state);
        let _ = write_done_sender.send(result);
    });

    let commit_result = match write_done_receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(result) => result,
        Err(error) => {
            let _ = resume_sender.send(());
            let _ = reader.join();
            let _ = writer.join();
            return Err(Box::new(error));
        }
    };
    commit_result?;
    resume_sender
        .send(())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let old_snapshot = reader
        .join()
        .map_err(|_| io::Error::other("snapshot reader thread panicked"))??;
    writer
        .join()
        .map_err(|_| io::Error::other("ledger writer thread panicked"))?;

    assert_eq!(old_snapshot.height, 5);
    assert_eq!(old_snapshot.balance, 10_000);
    assert_eq!(old_snapshot.nonce, 0);
    assert_eq!(old_snapshot.state_root, Some(old_root));

    let fresh_snapshot = store.read_snapshot()?;
    assert_eq!(fresh_snapshot.get_latest_height()?, 6);
    assert_eq!(
        fresh_snapshot
            .get_account(&sender_key)?
            .map(|account| account.balance),
        Some(8_999)
    );
    assert_eq!(
        fresh_snapshot
            .get_account(&recipient_key)?
            .map(|account| account.balance),
        Some(1_500)
    );
    assert_eq!(
        fresh_snapshot.get_latest_state_root()?,
        Some(expected_state.compute_state_root())
    );
    Ok(())
}

#[test]
fn l5_reopen_rehydrates_all_committed_blocks_and_latest_metadata() -> Result<(), Box<dyn Error>> {
    let directory = TempDir::new()?;
    let database_path = directory.path().join("recovery.redb");
    let account_key = [0x51; 32];
    let mut state = State::new();
    state.insert_account(account_key, Account::new(75_000, 4));
    let expected_state_root = state.compute_state_root();
    let expected_blocks = {
        let store = LedgerStore::open(&database_path)?;
        let mut previous_hash = [0; 32];
        let mut expected = Vec::new();
        for height in 0..=10 {
            let block = empty_block(height, previous_hash, expected_state_root);
            previous_hash = block.header.hash();
            store.commit_block(&block, &mut state)?;
            expected.push(block);
        }
        expected
    };

    let reopened = LedgerStore::open(&database_path)?;
    let snapshot = reopened.read_snapshot()?;
    assert_eq!(snapshot.get_latest_height()?, 10);
    assert_eq!(snapshot.get_latest_state_root()?, Some(expected_state_root));
    for expected in expected_blocks {
        assert_eq!(
            snapshot.get_block_by_height(expected.header.height)?,
            Some(expected)
        );
    }
    assert_eq!(
        snapshot.get_account(&account_key)?,
        Some(Account::new(75_000, 4))
    );
    Ok(())
}
