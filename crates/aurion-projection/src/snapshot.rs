//! Modul snapshot model baca untuk konsistensi titik-waktu.
//!
//! Modul ini menerapkan pencatatan state model baca pada ketinggian blok
//! tertentu dengan pelacakan ketertinggalan proyeksi (P2).

use aurion_criptografi::Hash256;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::ProjectionError;

/// Type alias untuk AccountId.
pub type AccountId = Hash256;

/// Type alias untuk Quanta (satuan saldo).
pub type Quanta = u64;

/// Snapshot model baca pada ketinggian blok tertentu.
#[derive(Debug, Clone)]
pub struct ReadSnapshot {
    /// Ketinggian blok snapshot.
    height: u64,
    /// Hash state root pada ketinggian tersebut.
    state_root: [u8; 32],
    /// Peta alamat ke saldo.
    balances: HashMap<AccountId, Quanta>,
}

impl ReadSnapshot {
    /// Buat snapshot baru.
    #[must_use]
    pub fn new(height: u64, state_root: [u8; 32]) -> Self {
        Self {
            height,
            state_root,
            balances: HashMap::new(),
        }
    }

    /// Dapatkan ketinggian snapshot.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Dapatkan hash state root.
    #[must_use]
    pub fn state_root(&self) -> &[u8; 32] {
        &self.state_root
    }

    /// Dapatkan saldo untuk sebuah alamat.
    #[must_use]
    pub fn get_balance(&self, account_id: &AccountId) -> Option<Quanta> {
        self.balances.get(account_id).copied()
    }

    /// Tambahkan saldo untuk sebuah alamat.
    pub fn set_balance(&mut self, account_id: AccountId, balance: Quanta) {
        self.balances.insert(account_id, balance);
    }

    /// Dapatkan seluruh peta saldo snapshot.
    ///
    /// Dipakai oleh verifikasi determinisme P4 untuk membandingkan
    /// state baca secara menyeluruh, bukan hanya skalar ringkasan.
    #[must_use]
    pub fn balances(&self) -> &HashMap<AccountId, Quanta> {
        &self.balances
    }

    /// Perbarui saldo untuk sebuah alamat.
    pub fn update_balance<F>(&mut self, account_id: AccountId, f: F)
    where
        F: FnOnce(Quanta) -> Quanta,
    {
        let balance = self.balances.get(&account_id).copied().unwrap_or(0);
        self.balances.insert(account_id, f(balance));
    }
}

/// Manajer snapshot yang menyimpan snapshot model baca.
#[derive(Debug)]
pub struct SnapshotManager {
    /// Ketinggian ledger utama terakhir yang diketahui.
    ledger_height: Arc<RwLock<u64>>,
    /// Ketinggian proyeksi saat ini.
    projection_height: Arc<RwLock<u64>>,
    /// Snapshot terbaru.
    latest_snapshot: Arc<RwLock<ReadSnapshot>>,
    /// Riwayat snapshot (opsional, untuk rollback).
    snapshot_history: Arc<RwLock<Vec<ReadSnapshot>>>,
}

impl SnapshotManager {
    /// Buat manajer snapshot baru.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ledger_height: Arc::new(RwLock::new(0)),
            projection_height: Arc::new(RwLock::new(0)),
            latest_snapshot: Arc::new(RwLock::new(ReadSnapshot::new(0, [0u8; 32]))),
            snapshot_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Perbarui ketinggian ledger utama.
    pub fn update_ledger_height(&self, height: u64) {
        let mut ledger_height = self.ledger_height.write().unwrap_or_else(|_| {
            panic!("SnapshotManager ledger_height mutex poisoned");
        });
        *ledger_height = height;
    }

    /// Perbarui ketinggian proyeksi.
    pub fn update_projection_height(&self, height: u64) {
        let mut projection_height = self.projection_height.write().unwrap_or_else(|_| {
            panic!("SnapshotManager projection_height mutex poisoned");
        });
        *projection_height = height;
    }

    /// Dapatkan ketinggian ledger utama.
    #[must_use]
    pub fn ledger_height(&self) -> u64 {
        *self.ledger_height.read().unwrap_or_else(|_| {
            panic!("SnapshotManager ledger_height mutex poisoned");
        })
    }

    /// Dapatkan ketinggian proyeksi.
    #[must_use]
    pub fn projection_height(&self) -> u64 {
        *self.projection_height.read().unwrap_or_else(|_| {
            panic!("SnapshotManager projection_height mutex poisoned");
        })
    }

    /// Hitung ketertinggalan proyeksi (lag).
    ///
    /// Lag = ledger_height - projection_height
    #[must_use]
    pub fn projection_lag(&self) -> u64 {
        let ledger_h = self.ledger_height();
        let proj_h = self.projection_height();
        ledger_h.saturating_sub(proj_h)
    }

    /// Dapatkan snapshot terbaru.
    #[must_use]
    pub fn latest_snapshot(&self) -> ReadSnapshot {
        self.latest_snapshot
            .read()
            .unwrap_or_else(|_| {
                panic!("SnapshotManager latest_snapshot mutex poisoned");
            })
            .clone()
    }

    /// Perbarui snapshot terbaru.
    pub fn update_latest_snapshot(&self, snapshot: ReadSnapshot) {
        let mut latest = self.latest_snapshot.write().unwrap_or_else(|_| {
            panic!("SnapshotManager latest_snapshot mutex poisoned");
        });

        let mut history = self.snapshot_history.write().unwrap_or_else(|_| {
            panic!("SnapshotManager snapshot_history mutex poisoned");
        });

        // Simpan snapshot lama ke riwayat
        history.push(latest.clone());
        *latest = snapshot;
    }

    /// Dapatkan snapshot pada ketinggian tertentu.
    ///
    /// # Arguments
    /// * `height` - Ketinggian blok snapshot
    ///
    /// # Returns
    /// `Ok(ReadSnapshot)` jika ditemukan, `Err(ProjectionError)` jika tidak.
    pub fn get_snapshot_at(&self, height: u64) -> Result<ReadSnapshot, ProjectionError> {
        let history = self.snapshot_history.read().unwrap_or_else(|_| {
            panic!("SnapshotManager snapshot_history mutex poisoned");
        });

        // Cari snapshot dengan ketinggian yang cocok
        for snapshot in history.iter() {
            if snapshot.height() == height {
                return Ok(snapshot.clone());
            }
        }

        // Cek snapshot terbaru
        let latest = self.latest_snapshot.read().unwrap_or_else(|_| {
            panic!("SnapshotManager latest_snapshot mutex poisoned");
        });

        if latest.height() == height {
            return Ok(latest.clone());
        }

        Err(ProjectionError::SnapshotNotFound { height })
    }

    /// Dapatkan saldo untuk sebuah alamat dari snapshot terbaru.
    #[must_use]
    pub fn get_balance(&self, account_id: &AccountId) -> Option<Quanta> {
        let snapshot = self.latest_snapshot.read().unwrap_or_else(|_| {
            panic!("SnapshotManager latest_snapshot mutex poisoned");
        });
        snapshot.get_balance(account_id)
    }

    /// Verifikasi konsistensi snapshot: `ledger_height` >= `projection_height`.
    ///
    /// # Returns
    /// `Ok(())` jika konsisten, `Err(ProjectionError)` jika tidak.
    /// # Errors
    /// Mengembalikan error jika tidak konsisten.
    pub fn verify_consistency(&self) -> Result<(), ProjectionError> {
        let ledger_h = self.ledger_height();
        let proj_h = self.projection_height();

        if ledger_h < proj_h {
            return Err(ProjectionError::LedgerHeightLags {
                ledger_height: ledger_h,
                projection_height: proj_h,
            });
        }

        Ok(())
    }

    /// Reset manajer snapshot.
    /// # Panics
    /// Akan panik jika lock diracuni.
    pub fn reset(&self) {
        let mut ledger_height = self.ledger_height.write().unwrap_or_else(|_| {
            panic!("SnapshotManager ledger_height mutex poisoned");
        });
        let mut projection_height = self.projection_height.write().unwrap_or_else(|_| {
            panic!("SnapshotManager projection_height mutex poisoned");
        });
        let mut latest_snapshot = self.latest_snapshot.write().unwrap_or_else(|_| {
            panic!("SnapshotManager latest_snapshot mutex poisoned");
        });
        let mut snapshot_history = self.snapshot_history.write().unwrap_or_else(|_| {
            panic!("SnapshotManager snapshot_history mutex poisoned");
        });

        *ledger_height = 0;
        *projection_height = 0;
        *latest_snapshot = ReadSnapshot::new(0, [0u8; 32]);
        snapshot_history.clear();
    }
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let snapshot = ReadSnapshot::new(100, [42u8; 32]);
        assert_eq!(snapshot.height(), 100);
    }

    #[test]
    fn test_snapshot_balance() {
        let alice: AccountId = [1u8; 32];
        let mut snapshot = ReadSnapshot::new(100, [42u8; 32]);

        snapshot.set_balance(alice, 1000);
        assert_eq!(snapshot.get_balance(&alice), Some(1000));
    }

    #[test]
    fn test_snapshot_manager_lag() {
        let manager = SnapshotManager::new();

        manager.update_ledger_height(100);
        manager.update_projection_height(95);

        assert_eq!(manager.projection_lag(), 5);
    }

    #[test]
    fn test_snapshot_manager_consistency() {
        let manager = SnapshotManager::new();

        // Consistent: `ledger_height` >= `projection_height`
        manager.update_ledger_height(100);
        manager.update_projection_height(95);
        assert!(manager.verify_consistency().is_ok());

        // Inconsistent: ledger_height < projection_height
        manager.update_ledger_height(90);
        manager.update_projection_height(95);
        assert!(matches!(
            manager.verify_consistency(),
            Err(ProjectionError::LedgerHeightLags { .. })
        ));
    }

    #[test]
    fn test_lag_zero_when_synced() {
        let manager = SnapshotManager::new();

        manager.update_ledger_height(100);
        manager.update_projection_height(100);

        assert_eq!(manager.projection_lag(), 0);
    }

    #[test]
    fn test_snapshot_update() {
        let manager = SnapshotManager::new();

        let snapshot1 = ReadSnapshot::new(1, [1u8; 32]);
        manager.update_latest_snapshot(snapshot1);

        assert_eq!(manager.latest_snapshot().height(), 1);

        let snapshot2 = ReadSnapshot::new(2, [2u8; 32]);
        manager.update_latest_snapshot(snapshot2);

        assert_eq!(manager.latest_snapshot().height(), 2);
    }

    #[test]
    fn test_get_snapshot_at() {
        let manager = SnapshotManager::new();

        let snapshot1 = ReadSnapshot::new(1, [1u8; 32]);
        manager.update_latest_snapshot(snapshot1);

        let snapshot2 = ReadSnapshot::new(2, [2u8; 32]);
        manager.update_latest_snapshot(snapshot2);

        // Should find snapshot at height 2 (latest)
        let found = manager.get_snapshot_at(2).unwrap();
        assert_eq!(found.height(), 2);

        // Should find snapshot at height 1 (in history)
        let found = manager.get_snapshot_at(1).unwrap();
        assert_eq!(found.height(), 1);

        // Should not find snapshot at height 3
        assert!(matches!(
            manager.get_snapshot_at(3),
            Err(ProjectionError::SnapshotNotFound { .. })
        ));
    }

    #[test]
    fn test_reset() {
        let manager = SnapshotManager::new();

        manager.update_ledger_height(100);
        manager.update_projection_height(95);
        let snapshot = ReadSnapshot::new(100, [42u8; 32]);
        manager.update_latest_snapshot(snapshot);

        assert_eq!(manager.ledger_height(), 100);
        assert_eq!(manager.projection_height(), 95);
        assert_eq!(manager.latest_snapshot().height(), 100);

        manager.reset();

        assert_eq!(manager.ledger_height(), 0);
        assert_eq!(manager.projection_height(), 0);
        assert_eq!(manager.latest_snapshot().height(), 0);
    }
}
