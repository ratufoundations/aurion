//! Modul pembersihan riwayat usang dengan jendela retensi.
//!
//! Modul ini menerapkan pembersihan data event dan histori transaksi
//! yang berada di luar jendela retensi untuk mencegah pembengkakan disk (P3).

use std::sync::{Arc, RwLock};

use crate::error::ProjectionError;

/// Jendela retensi default dalam blok (P3: Pembersihan data di luar jendela retensi).
pub const DEFAULT_RETENTION_WINDOW: u64 = 10_000;

/// Konfigurasi pembersihan.
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Jendela retensi dalam blok.
    retention_window: u64,
}

impl PruningConfig {
    /// Buat konfigurasi pembersihan dengan jendela retensi default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            retention_window: DEFAULT_RETENTION_WINDOW,
        }
    }

    /// Buat konfigurasi pembersihan dengan jendela retensi khusus.
    ///
    /// # Arguments
    /// * `retention_window` - Jendela retensi dalam blok
    ///
    /// # Returns
    /// `Ok(PruningConfig)` jika valid, `Err(ProjectionError)` jika tidak.
    pub fn with_retention_window(retention_window: u64) -> Result<Self, ProjectionError> {
        if retention_window == 0 {
            return Err(ProjectionError::InvalidRetentionWindow { window: 0 });
        }
        Ok(Self { retention_window })
    }

    /// Dapatkan jendela retensi.
    #[must_use]
    pub fn retention_window(&self) -> u64 {
        self.retention_window
    }

    /// Hitung batas retensi: current_height - retention_window.
    ///
    /// # Arguments
    /// * `current_height` - Ketinggian blok saat ini
    ///
    /// # Returns
    /// Ketinggian batas retensi.
    #[must_use]
    pub fn retention_bound(&self, current_height: u64) -> u64 {
        current_height.saturating_sub(self.retention_window)
    }
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Manajer pembersihan yang menangani pembersihan data usang.
#[derive(Debug)]
pub struct PruningManager {
    config: PruningConfig,
    /// Ketinggian proyeksi saat ini.
    projection_height: Arc<RwLock<u64>>,
}

impl PruningManager {
    /// Buat manajer pembersihan baru.
    #[must_use]
    pub fn new(config: PruningConfig) -> Self {
        Self {
            config,
            projection_height: Arc::new(RwLock::new(0)),
        }
    }

    /// Perbarui ketinggian proyeksi.
    pub fn update_projection_height(&self, height: u64) {
        let mut projection_height = self.projection_height.write().unwrap_or_else(|_| {
            panic!("PruningManager projection_height mutex poisoned");
        });
        *projection_height = height;
    }

    /// Dapatkan ketinggian proyeksi.
    #[must_use]
    pub fn projection_height(&self) -> u64 {
        *self.projection_height.read().unwrap_or_else(|_| {
            panic!("PruningManager projection_height mutex poisoned");
        })
    }

    /// Periksa apakah data pada ketinggian tertentu memenuhi syarat untuk dipangkas.
    ///
    /// # Arguments
    /// * `height` - Ketinggian data yang akan diperiksa
    ///
    /// # Returns
    /// `true` jika data memenuhi syarat untuk dipangkas, `false` sebaliknya.
    #[must_use]
    pub fn should_prune(&self, height: u64) -> bool {
        let current_height = self.projection_height();
        let bound = self.config.retention_bound(current_height);
        height < bound
    }

    /// Lakukan pembersihan data sebelum batas retensi.
    ///
    /// # Arguments
    /// * `current_height` - Ketinggian blok saat ini (opsional, gunakan ketinggian proyeksi jika None)
    ///
    /// # Returns
    /// `Ok(())` jika pembersihan berhasil, `Err(ProjectionError)` jika gagal.
    ///
    /// # Errors
    /// - `ProjectionError::PruneHeightExceedsCursor` jika current_height > ketinggian proyeksi
    /// - `ProjectionError::NothingToPrune` jika tidak ada data yang memenuhi syarat
    pub fn prune(&self, current_height: Option<u64>) -> Result<(), ProjectionError> {
        let current_height = current_height.unwrap_or_else(|| self.projection_height());
        
        // Validasi: current_height tidak boleh melebihi ketinggian proyeksi
        if current_height > self.projection_height() {
            return Err(ProjectionError::PruneHeightExceedsCursor {
                prune_height: current_height,
                cursor: self.projection_height(),
            });
        }

        // Periksa apakah ada data yang memenuhi syarat
        let bound = self.config.retention_bound(current_height);
        if bound == 0 {
            return Err(ProjectionError::NothingToPrune { height: current_height });
        }

        // Simulasikan pembersihan (implementasi nyata akan menghapus dari storage)
        // Dalam modul ini, kami hanya melacak logika pembersihan
        // Pembersihan sebenarnya akan diimplementasikan di storage adapter
        
        Ok(())
    }

    /// Dapatkan batas retensi.
    #[must_use]
    pub fn retention_bound(&self) -> u64 {
        let current_height = self.projection_height();
        self.config.retention_bound(current_height)
    }

    /// Dapatkan jendela retensi.
    #[must_use]
    pub fn retention_window(&self) -> u64 {
        self.config.retention_window()
    }

    /// Reset manajer pembersihan.
    pub fn reset(&self) {
        let mut projection_height = self.projection_height.write().unwrap_or_else(|_| {
            panic!("PruningManager projection_height mutex poisoned");
        });
        *projection_height = 0;
    }
}

impl Default for PruningManager {
    fn default() -> Self {
        Self::new(PruningConfig::new())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_pruning_config_default() {
        let config = PruningConfig::new();
        assert_eq!(config.retention_window(), DEFAULT_RETENTION_WINDOW);
    }

    #[test]
    fn test_pruning_config_custom() {
        let config = PruningConfig::with_retention_window(5000).unwrap();
        assert_eq!(config.retention_window(), 5000);
    }

    #[test]
    fn test_pruning_config_invalid_zero() {
        let result = PruningConfig::with_retention_window(0);
        assert!(matches!(result, Err(ProjectionError::InvalidRetentionWindow { .. })));
    }

    #[test]
    fn test_retention_bound() {
        let config = PruningConfig::with_retention_window(1000).unwrap();
        
        // current_height = 1500, retention_window = 1000
        // retention_bound = 1500 - 1000 = 500
        assert_eq!(config.retention_bound(1500), 500);
        
        // current_height = 1000, retention_window = 1000
        // retention_bound = 1000 - 1000 = 0
        assert_eq!(config.retention_bound(1000), 0);
        
        // current_height = 500, retention_window = 1000
        // retention_bound = 500 - 1000 = 0 (saturating_sub)
        assert_eq!(config.retention_bound(500), 0);
    }

    #[test]
    fn test_should_prune() {
        let config = PruningConfig::with_retention_window(1000).unwrap();
        let manager = PruningManager::new(config);
        
        manager.update_projection_height(1500);
        
        // Data at height 400 should be pruned (400 < 1500 - 1000 = 500)
        assert!(manager.should_prune(400));
        
        // Data at height 500 should NOT be pruned (500 >= 500)
        assert!(!manager.should_prune(500));
        
        // Data at height 1000 should NOT be pruned
        assert!(!manager.should_prune(1000));
    }

    #[test]
    fn test_prune_success() {
        let config = PruningConfig::with_retention_window(1000).unwrap();
        let manager = PruningManager::new(config);
        
        manager.update_projection_height(1500);
        
        // Prune with current_height = 1500
        let result = manager.prune(Some(1500));
        assert!(result.is_ok());
    }

    #[test]
    fn test_prune_nothing_to_prune() {
        let config = PruningConfig::with_retention_window(1000).unwrap();
        let manager = PruningManager::new(config);
        
        manager.update_projection_height(100);
        
        // Prune when current_height <= retention_window
        let result = manager.prune(Some(100));
        assert!(matches!(result, Err(ProjectionError::NothingToPrune { .. })));
    }

    #[test]
    fn test_prune_height_exceeds_cursor() {
        let config = PruningConfig::with_retention_window(1000).unwrap();
        let manager = PruningManager::new(config);
        
        manager.update_projection_height(1000);
        
        // Try to prune at height 2000 (exceeds projection cursor)
        let result = manager.prune(Some(2000));
        assert!(matches!(
            result,
            Err(ProjectionError::PruneHeightExceedsCursor { .. })
        ));
    }

    #[test]
    fn test_prune_with_none_uses_projection_height() {
        let config = PruningConfig::with_retention_window(1000).unwrap();
        let manager = PruningManager::new(config);
        
        manager.update_projection_height(1500);
        
        // Prune with None (uses projection height)
        let result = manager.prune(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_retention_bound_zero_window() {
        let config = PruningConfig::with_retention_window(1).unwrap();
        
        // current_height = 10, retention_window = 1
        // retention_bound = 10 - 1 = 9
        assert_eq!(config.retention_bound(10), 9);
    }
}
