//! Modul indeks transaksi berbasis alamat.
//!
//! Modul ini menerapkan indeksasi riwayat transaksi per alamat dengan
//! paginasi berbatas untuk mendukung kueri cepat tanpa memindai seluruh
//! riwayat blok (P1).

use aurion_core::Transaction;
use aurion_criptografi::Hash256;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::error::ProjectionError;

/// Type alias untuk AccountId.
pub type AccountId = Hash256;

/// Type alias untuk Quanta (satuan saldo).
pub type Quanta = u64;

/// Batas maksimal ukuran halaman (P1: Kueri riwayat dengan filter limit dan offset).
pub const MAX_PAGE_LIMIT: u64 = 100;

/// Entri riwayat transaksi untuk sebuah alamat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionEntry {
    /// Hash transaksi.
    pub tx_hash: [u8; 32],
    /// Ketinggian blok.
    pub block_height: u64,
    /// Apakah transaksi keluar (true) atau masuk (false).
    pub is_outbound: bool,
}

/// Indeks transaksi berbasis alamat.
///
/// Memetakan AccountId ke daftar transaksi terkait (sebagai pengirim maupun penerima).
#[derive(Debug)]
pub struct AddressIndex {
    /// Peta alamat ke riwayat transaksi.
    index: Arc<RwLock<BTreeMap<AccountId, Vec<TransactionEntry>>>>,
}

impl AddressIndex {
    /// Buat indeks alamat baru.
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Tambahkan transaksi ke indeks untuk pengirim dan penerima.
    ///
    /// # Arguments
    /// * `tx` - Transaksi yang akan diindeks
    /// * `tx_hash` - Hash transaksi
    /// * `block_height` - Ketinggian blok
    ///
    /// # Errors
    /// Tidak mengembalikan error, tetapi akan diabaikan jika transaksi tidak valid.
    pub fn add_transaction(&self, tx: &Transaction, tx_hash: [u8; 32], block_height: u64) {
        let mut index = self.index.write().unwrap_or_else(|_| {
            // Mutex poisoned, panik terkontrol
            panic!("AddressIndex mutex poisoned");
        });

        let entry = TransactionEntry {
            tx_hash,
            block_height,
            is_outbound: true,
        };

        // Tambahkan ke pengirim (outbound)
        let sender: AccountId = tx.sender;
        index.entry(sender).or_default().push(entry.clone());

        // Tambahkan ke penerima (inbound)
        let entry_inbound = TransactionEntry {
            tx_hash,
            block_height,
            is_outbound: false,
        };
        let recipient: AccountId = tx.recipient;
        index.entry(recipient).or_default().push(entry_inbound);
    }

    /// Dapatkan riwayat transaksi untuk sebuah alamat dengan paginasi.
    ///
    /// # Arguments
    /// * `account_id` - AccountId yang akan dikueri
    /// * `limit` - Batas jumlah entri (0 = gunakan default, > MAX_PAGE_LIMIT = normalisasi ke MAX_PAGE_LIMIT)
    /// * `offset` - Offset untuk paginasi
    ///
    /// # Returns
    /// `Ok(Vec<TransactionEntry>)` - Daftar entri transaksi, diurutkan dari terbaru ke tertua.
    ///
    /// # Errors
    /// - `ProjectionError::InvalidPaginationLimit` jika limit tidak valid
    /// - `ProjectionError::InvalidPaginationOffset` jika offset tidak valid
    pub fn get_transaction_history(
        &self,
        account_id: &AccountId,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<TransactionEntry>, ProjectionError> {
        // Validasi limit
        if limit == 0 {
            return Ok(vec![]);
        }

        if limit > MAX_PAGE_LIMIT {
            return Err(ProjectionError::InvalidPaginationLimit {
                limit,
                max: MAX_PAGE_LIMIT,
            });
        }

        // Validasi offset
        if offset > u64::MAX - limit {
            return Err(ProjectionError::InvalidPaginationOffset { offset });
        }

        let index = self.index.read().unwrap_or_else(|_| {
            panic!("AddressIndex mutex poisoned");
        });

        // Dapatkan riwayat untuk alamat
        let entries = index.get(account_id).cloned().unwrap_or_default();

        // Urutkan dari terbaru ke tertua (reverse chronological)
        let mut sorted_entries: Vec<TransactionEntry> = entries;
        sorted_entries.sort_by_key(|b| std::cmp::Reverse(b.block_height));

        // Terapkan paginasi
        let start = usize::try_from(offset).unwrap_or(0);
        let end = start + usize::try_from(limit).unwrap_or(MAX_PAGE_LIMIT as usize);

        if start >= sorted_entries.len() {
            return Ok(vec![]);
        }

        Ok(sorted_entries[start..end.min(sorted_entries.len())].to_vec())
    }

    /// Dapatkan total jumlah transaksi untuk sebuah alamat.
    #[must_use]
    pub fn get_transaction_count(&self, account_id: &AccountId) -> usize {
        let index = self.index.read().unwrap_or_else(|_| {
            panic!("AddressIndex mutex poisoned");
        });
        index.get(account_id).map_or(0, std::vec::Vec::len)
    }

    /// Dapatkan jumlah total transaksi keluar untuk sebuah alamat.
    #[must_use]
    pub fn get_outbound_count(&self, account_id: &AccountId) -> usize {
        let index = self.index.read().unwrap_or_else(|_| {
            panic!("AddressIndex mutex poisoned");
        });
        index.get(account_id).map_or(0, |entries| {
            entries.iter().filter(|e| e.is_outbound).count()
        })
    }

    /// Dapatkan jumlah total transaksi masuk untuk sebuah alamat.
    #[must_use]
    pub fn get_inbound_count(&self, account_id: &AccountId) -> usize {
        let index = self.index.read().unwrap_or_else(|_| {
            panic!("AddressIndex mutex poisoned");
        });
        index.get(account_id).map_or(0, |entries| {
            entries.iter().filter(|e| !e.is_outbound).count()
        })
    }

    /// Bersihkan indeks.
    pub fn clear(&self) {
        let mut index = self.index.write().unwrap_or_else(|_| {
            panic!("AddressIndex mutex poisoned");
        });
        index.clear();
    }
}

impl Default for AddressIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use crate::{AddressIndex, ProjectionError, MAX_PAGE_LIMIT};
    use aurion_core::Transaction;
    use aurion_criptografi::Hash256 as AccountId;

    #[test]
    fn test_address_index_creation() {
        let index = AddressIndex::new();
        assert_eq!(index.get_transaction_count(&AccountId::default()), 0);
    }

    #[test]
    fn test_add_transaction() {
        let index = AddressIndex::new();

        // Create test accounts
        let alice = [1u8; 32];
        let bob = [2u8; 32];

        // Create a transaction from Alice to Bob
        let tx = Transaction::new(alice, bob, 100, 0, 0, [0u8; 64]);

        index.add_transaction(&tx, [1u8; 32], 1);

        // Alice should have 1 outbound transaction
        assert_eq!(index.get_outbound_count(&alice), 1);
        // Bob should have 1 inbound transaction
        assert_eq!(index.get_inbound_count(&bob), 1);
        // Total for Alice should be 1
        assert_eq!(index.get_transaction_count(&alice), 1);
        // Total for Bob should be 1
        assert_eq!(index.get_transaction_count(&bob), 1);
    }

    #[test]
    fn test_transaction_history_pagination() {
        let index = AddressIndex::new();

        let alice = [1u8; 32];
        let bob = [2u8; 32];

        // Add multiple transactions
        for i in 1..=10 {
            let tx = Transaction::new(alice, bob, 100, i, 0, [0u8; 64]);
            let hash = [u8::try_from(i % 256).unwrap(); 32];
            index.add_transaction(&tx, hash, i);
        }

        // Get history with limit
        let history = index.get_transaction_history(&alice, 5, 0).unwrap();
        assert_eq!(history.len(), 5);

        // Check ordering (should be reverse chronological)
        for (i, entry) in history.iter().enumerate() {
            // The first entry should be height 10, second 9, etc.
            assert_eq!(entry.block_height, 10 - u64::try_from(i).unwrap());
        }
    }

    #[test]
    fn test_empty_history_for_new_address() {
        let index = AddressIndex::new();
        let alice = [1u8; 32];

        let history = index.get_transaction_history(&alice, 10, 0).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_invalid_pagination_limit() {
        let index = AddressIndex::new();
        let alice = [1u8; 32];

        // Limit exceeds MAX_PAGE_LIMIT
        let result = index.get_transaction_history(&alice, MAX_PAGE_LIMIT + 1, 0);
        assert!(matches!(
            result,
            Err(ProjectionError::InvalidPaginationLimit { .. })
        ));
    }

    #[test]
    fn test_zero_limit_returns_empty() {
        let index = AddressIndex::new();
        let alice = [1u8; 32];

        let history = index.get_transaction_history(&alice, 0, 0).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_clear() {
        let index = AddressIndex::new();

        let alice = [1u8; 32];
        let bob = [2u8; 32];

        let tx = Transaction::new(alice, bob, 100, 0, 0, [0u8; 64]);
        index.add_transaction(&tx, [1u8; 32], 1);

        assert_eq!(index.get_transaction_count(&alice), 1);

        index.clear();

        assert_eq!(index.get_transaction_count(&alice), 0);
    }
}
