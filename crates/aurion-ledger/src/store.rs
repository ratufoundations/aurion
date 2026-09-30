use crate::{
    codec::Codec,
    error::LedgerError,
    schema::{ACCOUNTS_TABLE, BLOCKS_TABLE, BLOCK_INDEX_TABLE, METADATA_TABLE, MODULE_KV_TABLE},
};
use aurion_core::{Account, Block, State, PROTOCOL_FEE_SINK};
use aurion_criptografi::{Hash256, PublicKeyBytes};
use redb::{Database, ReadTransaction, ReadableTable, Table};
use std::{path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct LedgerStore {
    db: Arc<Database>,
}

/// Snapshot baca konsisten yang mempertahankan versi ledger saat dibuka.
#[derive(Debug)]
pub struct LedgerSnapshot {
    read_txn: ReadTransaction,
}

impl LedgerStore {
    /// Inisialisasi basis data ledger pada file storage.
    ///
    /// # Errors
    /// Mengembalikan error bila file database atau tabel ledger gagal dibuat.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let db = Database::create(path).map_err(|e| LedgerError::DatabaseError(e.to_string()))?;
        let write_txn = db
            .begin_write()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
        {
            let _ = write_txn
                .open_table(ACCOUNTS_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let _ = write_txn
                .open_table(BLOCKS_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let _ = write_txn
                .open_table(BLOCK_INDEX_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let _ = write_txn
                .open_table(METADATA_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let _ = write_txn
                .open_table(MODULE_KV_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|e| LedgerError::CommitError(e.to_string()))?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Membuka snapshot baca untuk beberapa kueri yang harus melihat versi sama.
    ///
    /// # Errors
    /// Mengembalikan error bila transaksi baca gagal dimulai.
    pub fn read_snapshot(&self) -> Result<LedgerSnapshot, LedgerError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
        Ok(LedgerSnapshot { read_txn })
    }

    /// Ambil data status akun terkini langsung dari disk.
    ///
    /// # Errors
    /// Mengembalikan error bila transaksi baca, tabel, atau data akun gagal dibaca.
    pub fn get_account(&self, pubkey: &PublicKeyBytes) -> Result<Option<Account>, LedgerError> {
        self.read_snapshot()?.get_account(pubkey)
    }

    /// Ambil blok berdasarkan nomor tinggi (Block Height).
    ///
    /// # Errors
    /// Mengembalikan error bila transaksi baca, tabel, atau data blok gagal dibaca.
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, LedgerError> {
        self.read_snapshot()?.get_block_by_height(height)
    }

    /// Ambil nomor tinggi blok terakhir yang sudah tersimpan permanen.
    ///
    /// # Errors
    /// Mengembalikan error bila metadata ledger tidak dapat dibaca atau rusak.
    pub fn get_latest_height(&self) -> Result<u64, LedgerError> {
        self.read_snapshot()?.get_latest_height()
    }

    /// Ambil state root terakhir yang dicatat dalam metadata.
    ///
    /// # Errors
    /// Mengembalikan error bila metadata rusak atau gagal dibaca.
    pub fn get_latest_state_root(&self) -> Result<Option<Hash256>, LedgerError> {
        self.read_snapshot()?.get_latest_state_root()
    }

    /// Commit blok append-only secara atomik ke state memory dan seluruh tabel ledger.
    ///
    /// # Errors
    /// Mengembalikan error bila tinggi blok bukan urutan berikutnya, blok sudah ada,
    /// eksekusi state gagal, atau transaksi ACID redb gagal.
    pub fn commit_block(&self, block: &Block, state: &mut State) -> Result<(), LedgerError> {
        tracing::info!(
            height = block.header.height,
            txs = block.transactions.len(),
            "Menulis blok permanen ke redb"
        );

        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
        let mut next_state = state.clone();
        {
            let mut accounts_table = write_txn
                .open_table(ACCOUNTS_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let mut blocks_table = write_txn
                .open_table(BLOCKS_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let mut index_table = write_txn
                .open_table(BLOCK_INDEX_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;
            let mut meta_table = write_txn
                .open_table(METADATA_TABLE)
                .map_err(|e| LedgerError::TableError(e.to_string()))?;

            validate_block_position(block, &blocks_table, &meta_table)?;

            // Eksekusi pada salinan: error atau abort redb tidak mencemari state pemanggil.
            block.execute(&mut next_state)?;

            if block.header.height == 0 && block.transactions.is_empty() {
                for (pubkey, account) in next_state.accounts() {
                    let encoded = Codec::encode_account(account);
                    accounts_table
                        .insert(pubkey, &encoded)
                        .map_err(|e| LedgerError::StorageError(e.to_string()))?;
                }
            } else {
                for tx in &block.transactions {
                    let fee_recipient = if block.header.proposer == PROTOCOL_FEE_SINK {
                        PROTOCOL_FEE_SINK
                    } else {
                        block.header.proposer
                    };
                    for pubkey in [tx.sender, tx.recipient, fee_recipient] {
                        if let Some(account) = next_state.get_account(&pubkey) {
                            let encoded = Codec::encode_account(account);
                            accounts_table
                                .insert(&pubkey, &encoded)
                                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
                        }
                    }
                }
            }

            let block_bytes = Codec::encode_block(block);
            let block_hash = block.header.hash();
            blocks_table
                .insert(block.header.height, block_bytes.as_slice())
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
            index_table
                .insert(&block_hash, block.header.height)
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
            meta_table
                .insert("latest_height", &block.header.height.to_le_bytes()[..])
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
            meta_table
                .insert("latest_state_root", &block.header.state_root[..])
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
        }

        write_txn.commit().map_err(|e| {
            tracing::error!(height = block.header.height, error = %e, "Gagal commit blok ke disk");
            LedgerError::CommitError(e.to_string())
        })?;
        *state = next_state;
        Ok(())
    }
}

impl LedgerSnapshot {
    /// Membaca akun dari versi database pada saat snapshot dibuka.
    ///
    /// # Errors
    /// Mengembalikan error bila tabel atau data akun gagal dibaca.
    pub fn get_account(&self, pubkey: &PublicKeyBytes) -> Result<Option<Account>, LedgerError> {
        let table = self
            .read_txn
            .open_table(ACCOUNTS_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        let account = table
            .get(pubkey)
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
            .map(|value| {
                let bytes: [u8; 24] = *value.value();
                Codec::decode_account(&bytes)
            });
        Ok(account)
    }

    /// Membaca blok dari versi database pada saat snapshot dibuka.
    ///
    /// # Errors
    /// Mengembalikan error bila tabel atau data blok gagal dibaca.
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, LedgerError> {
        let table = self
            .read_txn
            .open_table(BLOCKS_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        table
            .get(height)
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
            .map(|value| Codec::decode_block(value.value()))
            .transpose()
    }

    /// Membaca latest height dari versi database pada saat snapshot dibuka.
    ///
    /// # Errors
    /// Mengembalikan error bila metadata rusak atau gagal dibaca.
    pub fn get_latest_height(&self) -> Result<u64, LedgerError> {
        let table = self
            .read_txn
            .open_table(METADATA_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        table
            .get("latest_height")
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
            .map(|value| decode_height(value.value()))
            .transpose()
            .map(|height| height.unwrap_or(0))
    }

    /// Membaca state root terakhir dari versi database pada saat snapshot dibuka.
    ///
    /// # Errors
    /// Mengembalikan error bila metadata memiliki panjang tidak valid atau gagal dibaca.
    pub fn get_latest_state_root(&self) -> Result<Option<Hash256>, LedgerError> {
        let table = self
            .read_txn
            .open_table(METADATA_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        table
            .get("latest_state_root")
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
            .map(|value| {
                value
                    .value()
                    .try_into()
                    .map_err(|_| LedgerError::MalformedData)
            })
            .transpose()
    }
}

fn validate_block_position(
    block: &Block,
    blocks_table: &Table<'_, u64, &[u8]>,
    meta_table: &Table<'_, &str, &[u8]>,
) -> Result<(), LedgerError> {
    if blocks_table
        .get(block.header.height)
        .map_err(|e| LedgerError::StorageError(e.to_string()))?
        .is_some()
    {
        return Err(LedgerError::BlockAlreadyExists(block.header.height));
    }

    let latest_height = meta_table
        .get("latest_height")
        .map_err(|e| LedgerError::StorageError(e.to_string()))?
        .map(|value| decode_height(value.value()))
        .transpose()?;
    let expected_height = match latest_height {
        Some(height) => height.checked_add(1).ok_or(LedgerError::HeightOverflow)?,
        None => 0,
    };
    if block.header.height != expected_height {
        return Err(LedgerError::NonSequentialBlock {
            expected: expected_height,
            actual: block.header.height,
        });
    }

    let expected_prev_hash = if block.header.height == 0 {
        [0; 32]
    } else {
        let previous_height = block.header.height - 1;
        let previous_block = blocks_table
            .get(previous_height)
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
            .ok_or(LedgerError::BlockNotFound(previous_height))?;
        let previous_header = Codec::decode_block(previous_block.value())?.header;
        if block.header.timestamp <= previous_header.timestamp {
            return Err(LedgerError::TimestampNotMonotonic);
        }
        previous_header.hash()
    };
    if block.header.prev_hash != expected_prev_hash {
        return Err(LedgerError::PreviousHashMismatch(block.header.height));
    }
    Ok(())
}

fn decode_height(bytes: &[u8]) -> Result<u64, LedgerError> {
    let height_bytes: [u8; 8] = bytes.try_into().map_err(|_| LedgerError::MalformedData)?;
    Ok(u64::from_le_bytes(height_bytes))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use aurion_core::{BlockHeader, Transaction};
    use aurion_criptografi::Keypair;
    use tempfile::NamedTempFile;

    #[test]
    fn test_atomic_ledger_commit_and_persistence() {
        let temp_file = NamedTempFile::new().expect("test operation should succeed");
        let store = LedgerStore::open(temp_file.path()).expect("test operation should succeed");
        let mut state = State::new();
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let alice_pk = alice.public_key_bytes();
        let bob_pk = bob.public_key_bytes();
        state.insert_account(alice_pk, Account::new(1_000_000, 0));
        let genesis = Block {
            header: aurion_core::BlockHeader {
                height: 0,
                prev_hash: [0; 32],
                state_root: state.compute_state_root(),
                tx_count: 0,
                timestamp: 0,
                proposer: [0; 32],
            },
            transactions: Vec::new(),
        };
        store
            .commit_block(&genesis, &mut state)
            .expect("test operation should succeed");
        let unsigned = Transaction::new(alice_pk, bob_pk, 200_000, 0, 1, [0u8; 64]);
        let sig = alice.sign(&unsigned.digest());
        let tx = Transaction::new(alice_pk, bob_pk, 200_000, 0, 1, sig);
        let mut shadow = state.clone();
        shadow
            .apply_transaction(&tx)
            .expect("test operation should succeed");
        let expected_root = shadow.compute_state_root();
        let block = Block {
            header: BlockHeader {
                height: 1,
                prev_hash: genesis.header.hash(),
                state_root: expected_root,
                tx_count: 1,
                timestamp: 1_000,
                proposer: [0; 32],
            },
            transactions: vec![tx],
        };
        store
            .commit_block(&block, &mut state)
            .expect("test operation should succeed");
        let alice_disk = store
            .get_account(&alice_pk)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        let bob_disk = store
            .get_account(&bob_pk)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        assert_eq!(alice_disk.balance, 799_999);
        assert_eq!(alice_disk.nonce, 1);
        assert_eq!(bob_disk.balance, 200_000);
        assert_eq!(
            store
                .get_latest_height()
                .expect("test operation should succeed"),
            1
        );
        let got = store
            .get_block_by_height(1)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        assert_eq!(got.header.height, 1);
        assert_eq!(got.transactions.len(), 1);
    }
}
