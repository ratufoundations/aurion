use crate::{
    codec::Codec,
    error::LedgerError,
    schema::{ACCOUNTS_TABLE, BLOCKS_TABLE, BLOCK_INDEX_TABLE, METADATA_TABLE},
};
use aurion_core::{Account, Block, State};
use redb::Database;
use std::path::Path;

pub struct LedgerStore {
    db: Database,
}

impl LedgerStore {
    /// Inisialisasi basis data ledger pada file storage
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
        }
        write_txn
            .commit()
            .map_err(|e| LedgerError::CommitError(e.to_string()))?;
        Ok(Self { db })
    }

    /// Ambil data status akun terkini langsung dari disk
    pub fn get_account(
        &self,
        pubkey: &aurion_criptografi::PublicKeyBytes,
    ) -> Result<Option<Account>, LedgerError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
        let table = read_txn
            .open_table(ACCOUNTS_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        if let Some(val) = table
            .get(pubkey)
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
        {
            let bytes: [u8; 16] = *val.value();
            Ok(Some(Codec::decode_account(&bytes)))
        } else {
            Ok(None)
        }
    }

    /// Ambil blok berdasarkan nomor tinggi (Block Height)
    pub fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, LedgerError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
        let table = read_txn
            .open_table(BLOCKS_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        if let Some(val) = table
            .get(height)
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
        {
            let block = Codec::decode_block(val.value())?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Ambil nomor tinggi blok terakhir yang sudah tersimpan permanen
    pub fn get_latest_height(&self) -> Result<u64, LedgerError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
        let table = read_txn
            .open_table(METADATA_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        if let Some(val) = table
            .get("latest_height")
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
        {
            let bytes = val.value();
            Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
        } else {
            Ok(0)
        }
    }

    /// COMMIT BLOK ATOMIK: Eksekusi transaksi, perbarui saldo, dan simpan blok secara ACID
    pub fn commit_block(&self, block: &Block, state: &mut State) -> Result<(), LedgerError> {
        // 1. Eksekusi blok terhadap State FSM in-memory
        block.execute(state)?;

        // 2. Buka Write Transaction tunggal (ACID - rollback otomatis jika error)
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
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

            // Simpan pembaruan saldo akun dari transaksi blok ini ke disk
            // Jika blok adalah Genesis (tinggi 0) tanpa transaksi, simpan seluruh akun state
            if block.header.height == 0 && block.transactions.is_empty() {
                for (pubkey, acc) in state.accounts() {
                    let enc = Codec::encode_account(acc);
                    accounts_table
                        .insert(pubkey, &enc)
                        .map_err(|e| LedgerError::StorageError(e.to_string()))?;
                }
            } else {
                for tx in &block.transactions {
                    if let Some(sender_acc) = state.get_account(&tx.sender) {
                        let enc = Codec::encode_account(sender_acc);
                        accounts_table
                            .insert(&tx.sender, &enc)
                            .map_err(|e| LedgerError::StorageError(e.to_string()))?;
                    }
                    if let Some(recipient_acc) = state.get_account(&tx.recipient) {
                        let enc = Codec::encode_account(recipient_acc);
                        accounts_table
                            .insert(&tx.recipient, &enc)
                            .map_err(|e| LedgerError::StorageError(e.to_string()))?;
                    }
                }
            }

            // Simpan bita blok dan indeks hash blok
            let block_bytes = Codec::encode_block(block);
            let block_hash = block.header.hash();

            blocks_table
                .insert(block.header.height, block_bytes.as_slice())
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
            index_table
                .insert(&block_hash, block.header.height)
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;

            // Perbarui metadata rantai
            meta_table
                .insert("latest_height", &block.header.height.to_le_bytes()[..])
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
            meta_table
                .insert("latest_state_root", &block.header.state_root[..])
                .map_err(|e| LedgerError::StorageError(e.to_string()))?;
        }

        // Commit fisik ke piringan disk
        write_txn
            .commit()
            .map_err(|e| LedgerError::CommitError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_core::{BlockHeader, Transaction};
    use aurion_criptografi::Keypair;
    use tempfile::NamedTempFile;

    #[test]
    fn test_atomic_ledger_commit_and_persistence() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = LedgerStore::open(temp_file.path()).unwrap();
        let mut state = State::new();
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let alice_pk = alice.public_key_bytes();
        let bob_pk = bob.public_key_bytes();
        state.insert_account(alice_pk, Account::new(1_000_000, 0));
        let unsigned = Transaction::new(alice_pk, bob_pk, 200_000, 0, [0u8; 64]);
        let sig = alice.sign(&unsigned.digest());
        let tx = Transaction::new(alice_pk, bob_pk, 200_000, 0, sig);
        let mut shadow = state.clone();
        shadow.apply_transaction(&tx).unwrap();
        let expected_root = shadow.compute_state_root();
        let block = Block {
            header: BlockHeader {
                height: 1,
                prev_hash: [0u8; 32],
                state_root: expected_root,
                tx_count: 1,
            },
            transactions: vec![tx],
        };
        store.commit_block(&block, &mut state).unwrap();
        let alice_disk = store.get_account(&alice_pk).unwrap().unwrap();
        let bob_disk = store.get_account(&bob_pk).unwrap().unwrap();
        assert_eq!(alice_disk.balance, 800_000);
        assert_eq!(alice_disk.nonce, 1);
        assert_eq!(bob_disk.balance, 200_000);
        assert_eq!(store.get_latest_height().unwrap(), 1);
        let got = store.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(got.header.height, 1);
        assert_eq!(got.transactions.len(), 1);
    }
}
