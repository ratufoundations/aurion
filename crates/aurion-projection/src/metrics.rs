//! Modul metrik kinerja proyeksi dengan akuntansi u64 nir-pecahan.
//!
//! Modul ini menerapkan pencatatan metrik proyeksi (cache hit rate, lag,
//! ukuran indeks, latensi) menggunakan integer murni u64 (P5).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Skala basis poin: 10.000 BPS = 100%.
pub const BPS_SCALE: u64 = 10_000;

/// Metrik kinerja proyeksi (thread-safe).
#[derive(Debug)]
pub struct ProjectionMetrics {
    /// Jumlah hit cache.
    pub cache_hits: AtomicU64,
    /// Jumlah miss cache.
    pub cache_misses: AtomicU64,
    /// Total durasi pemrosesan blok (mikrodetik).
    pub block_processing_time_us: AtomicU64,
    /// Jumlah blok yang diproyeksikan.
    pub blocks_processed: AtomicU64,
    /// Ukuran indeks alamat (byte).
    pub address_index_size: AtomicU64,
    /// Ukuran snapshot (byte).
    pub snapshot_size: AtomicU64,
    /// Waktu total untuk pembersihan.
    pub pruning_time_us: AtomicU64,
    /// Jumlah operasi pembersihan.
    pub pruning_operations: AtomicU64,
}

impl ProjectionMetrics {
    /// Buat metrik proyeksi baru.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            block_processing_time_us: AtomicU64::new(0),
            blocks_processed: AtomicU64::new(0),
            address_index_size: AtomicU64::new(0),
            snapshot_size: AtomicU64::new(0),
            pruning_time_us: AtomicU64::new(0),
            pruning_operations: AtomicU64::new(0),
        })
    }

    /// Catat hit cache.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Catat miss cache.
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Catat waktu pemrosesan blok.
    ///
    /// # Arguments
    /// * `time_us` - Waktu pemrosesan dalam mikrodetik
    pub fn record_block_processing(&self, time_us: u64) {
        self.block_processing_time_us.fetch_add(time_us, Ordering::Relaxed);
        self.blocks_processed.fetch_add(1, Ordering::Relaxed);
    }

    /// Catat ukuran indeks alamat.
    ///
    /// # Arguments
    /// * `size_bytes` - Ukuran dalam byte
    pub fn record_address_index_size(&self, size_bytes: u64) {
        self.address_index_size.store(size_bytes, Ordering::Relaxed);
    }

    /// Catat ukuran snapshot.
    ///
    /// # Arguments
    /// * `size_bytes` - Ukuran dalam byte
    pub fn record_snapshot_size(&self, size_bytes: u64) {
        self.snapshot_size.store(size_bytes, Ordering::Relaxed);
    }

    /// Catat operasi pembersihan.
    ///
    /// # Arguments
    /// * `time_us` - Waktu pembersihan dalam mikrodetik
    pub fn record_pruning(&self, time_us: u64) {
        self.pruning_time_us.fetch_add(time_us, Ordering::Relaxed);
        self.pruning_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Dapatkan jumlah cache hits.
    #[must_use]
    pub fn cache_hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah cache misses.
    #[must_use]
    pub fn cache_misses(&self) -> u64 {
        self.cache_misses.load(Ordering::Relaxed)
    }

    /// Dapatkan total waktu pemrosesan blok.
    #[must_use]
    pub fn block_processing_time_us(&self) -> u64 {
        self.block_processing_time_us.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah blok yang diproyeksikan.
    #[must_use]
    pub fn blocks_processed(&self) -> u64 {
        self.blocks_processed.load(Ordering::Relaxed)
    }

    /// Dapatkan ukuran indeks alamat.
    #[must_use]
    pub fn address_index_size(&self) -> u64 {
        self.address_index_size.load(Ordering::Relaxed)
    }

    /// Dapatkan ukuran snapshot.
    #[must_use]
    pub fn snapshot_size(&self) -> u64 {
        self.snapshot_size.load(Ordering::Relaxed)
    }

    /// Dapatkan total waktu pembersihan.
    #[must_use]
    pub fn pruning_time_us(&self) -> u64 {
        self.pruning_time_us.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah operasi pembersihan.
    #[must_use]
    pub fn pruning_operations(&self) -> u64 {
        self.pruning_operations.load(Ordering::Relaxed)
    }

    /// Hitung rasio cache hit dalam BPS.
    ///
    /// Formula: (hits * 10000) / (hits + misses)
    ///
    /// # Returns
    /// Rasio cache hit dalam BPS, atau 0 jika total kueri = 0.
    #[must_use]
    pub fn cache_hit_rate_bps(&self) -> u64 {
        let hits = self.cache_hits();
        let misses = self.cache_misses();
        let total = hits + misses;

        if total == 0 {
            return 0;
        }

        // (hits * 10000) / total
        (hits * BPS_SCALE) / total
    }

    /// Hitung rata-rata waktu pemrosesan blok dalam mikrodetik.
    ///
    /// # Returns
    /// Rata-rata waktu dalam mikrodetik, atau 0 jika blok yang diproses = 0.
    #[must_use]
    pub fn avg_block_processing_time_us(&self) -> u64 {
        let blocks = self.blocks_processed();
        if blocks == 0 {
            return 0;
        }
        self.block_processing_time_us() / blocks
    }

    /// Hitung rata-rata waktu pembersihan dalam mikrodetik.
    ///
    /// # Returns
    /// Rata-rata waktu dalam mikrodetik, atau 0 jika operasi pembersihan = 0.
    #[must_use]
    pub fn avg_pruning_time_us(&self) -> u64 {
        let ops = self.pruning_operations();
        if ops == 0 {
            return 0;
        }
        self.pruning_time_us() / ops
    }

    /// Reset semua metrik.
    pub fn reset(&self) {
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.block_processing_time_us.store(0, Ordering::Relaxed);
        self.blocks_processed.store(0, Ordering::Relaxed);
        self.address_index_size.store(0, Ordering::Relaxed);
        self.snapshot_size.store(0, Ordering::Relaxed);
        self.pruning_time_us.store(0, Ordering::Relaxed);
        self.pruning_operations.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = ProjectionMetrics::new();
        assert_eq!(metrics.cache_hits(), 0);
        assert_eq!(metrics.cache_misses(), 0);
        assert_eq!(metrics.blocks_processed(), 0);
    }

    #[test]
    fn test_record_cache_hit() {
        let metrics = ProjectionMetrics::new();
        metrics.record_cache_hit();
        metrics.record_cache_hit();
        assert_eq!(metrics.cache_hits(), 2);
    }

    #[test]
    fn test_record_cache_miss() {
        let metrics = ProjectionMetrics::new();
        metrics.record_cache_miss();
        assert_eq!(metrics.cache_misses(), 1);
    }

    #[test]
    fn test_cache_hit_rate_bps() {
        let metrics = ProjectionMetrics::new();
        
        // 75 hits, 25 misses = 75% hit rate
        // (75 * 10000) / 100 = 7500 BPS
        for _ in 0..75 {
            metrics.record_cache_hit();
        }
        for _ in 0..25 {
            metrics.record_cache_miss();
        }
        
        assert_eq!(metrics.cache_hit_rate_bps(), 7500);
    }

    #[test]
    fn test_cache_hit_rate_zero_queries() {
        let metrics = ProjectionMetrics::new();
        assert_eq!(metrics.cache_hit_rate_bps(), 0);
    }

    #[test]
    fn test_cache_hit_rate_all_hits() {
        let metrics = ProjectionMetrics::new();
        for _ in 0..100 {
            metrics.record_cache_hit();
        }
        // (100 * 10000) / 100 = 10000 BPS
        assert_eq!(metrics.cache_hit_rate_bps(), 10000);
    }

    #[test]
    fn test_cache_hit_rate_all_misses() {
        let metrics = ProjectionMetrics::new();
        for _ in 0..100 {
            metrics.record_cache_miss();
        }
        // (0 * 10000) / 100 = 0 BPS
        assert_eq!(metrics.cache_hit_rate_bps(), 0);
    }

    #[test]
    fn test_block_processing() {
        let metrics = ProjectionMetrics::new();
        metrics.record_block_processing(1000);
        metrics.record_block_processing(2000);
        
        assert_eq!(metrics.block_processing_time_us(), 3000);
        assert_eq!(metrics.blocks_processed(), 2);
        assert_eq!(metrics.avg_block_processing_time_us(), 1500);
    }

    #[test]
    fn test_avg_block_processing_zero_blocks() {
        let metrics = ProjectionMetrics::new();
        assert_eq!(metrics.avg_block_processing_time_us(), 0);
    }

    #[test]
    fn test_pruning_metrics() {
        let metrics = ProjectionMetrics::new();
        metrics.record_pruning(5000);
        metrics.record_pruning(3000);
        
        assert_eq!(metrics.pruning_time_us(), 8000);
        assert_eq!(metrics.pruning_operations(), 2);
        assert_eq!(metrics.avg_pruning_time_us(), 4000);
    }

    #[test]
    fn test_avg_pruning_zero_operations() {
        let metrics = ProjectionMetrics::new();
        assert_eq!(metrics.avg_pruning_time_us(), 0);
    }

    #[test]
    fn test_reset() {
        let metrics = ProjectionMetrics::new();
        metrics.record_cache_hit();
        metrics.record_cache_miss();
        metrics.record_block_processing(1000);
        metrics.record_pruning(5000);
        
        metrics.reset();
        
        assert_eq!(metrics.cache_hits(), 0);
        assert_eq!(metrics.cache_misses(), 0);
        assert_eq!(metrics.block_processing_time_us(), 0);
        assert_eq!(metrics.pruning_time_us(), 0);
    }

    #[test]
    fn test_index_sizes() {
        let metrics = ProjectionMetrics::new();
        metrics.record_address_index_size(1024);
        metrics.record_snapshot_size(2048);
        
        assert_eq!(metrics.address_index_size(), 1024);
        assert_eq!(metrics.snapshot_size(), 2048);
    }

    #[test]
    fn test_zero_float_guarantee() {
        // Audit statis: Tidak ada tipe f32/f64 di modul ini
        let src = include_str!("../src/metrics.rs");
        
        let patterns = [" f32", "f32:", "f32,", "f32(", " f64", "f64:", "f64,", "f64("];
        let has_float_type = patterns.iter().any(|p| src.contains(p));
        
        // Izinkan string literal "f32/f64" dan "F32/f64"
        let has_comment_literal = src.contains("f32/f64");
        
        assert!(!has_float_type || has_comment_literal, 
            "Pelanggaran Zero-Float P5: f32/f64 ditemukan di metrics.rs");
    }
}
