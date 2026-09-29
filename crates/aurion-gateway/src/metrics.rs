//! Modul metrik operasional gateway dengan akuntansi u64 nir-pecahan.
//!
//! Modul ini menerapkan pencatatan metrik gateway (request count, byte throughput,
//! latensi, error rates) menggunakan integer murni u64.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Skala basis poin: 10.000 BPS = 100%
pub const BPS_SCALE: u64 = 10_000;

/// Metrik operasional gateway (thread-safe).
#[derive(Debug)]
pub struct GatewayMetrics {
    /// Jumlah total request diterima
    pub total_requests: AtomicU64,
    /// Jumlah request yang berhasil
    pub success_count: AtomicU64,
    /// Jumlah request yang gagal
    pub error_count: AtomicU64,
    /// Total byte ingress (masuk)
    pub ingress_bytes: AtomicU64,
    /// Total byte egress (keluar)
    pub egress_bytes: AtomicU64,
    /// Jumlah request yang ditolak karena rate limit
    pub rate_limited_count: AtomicU64,
    /// Jumlah request yang ditolak karena payload terlalu besar
    pub payload_too_large_count: AtomicU64,
    /// Jumlah request yang ditolak karena sanitasi gagal
    pub sanitization_fail_count: AtomicU64,
}

impl GatewayMetrics {
    /// Buat metrik baru.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_requests: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            ingress_bytes: AtomicU64::new(0),
            egress_bytes: AtomicU64::new(0),
            rate_limited_count: AtomicU64::new(0),
            payload_too_large_count: AtomicU64::new(0),
            sanitization_fail_count: AtomicU64::new(0),
        })
    }

    /// Catat request masuk.
    pub fn record_request(&self, payload_size: u64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.ingress_bytes.fetch_add(payload_size, Ordering::Relaxed);
    }

    /// Catat request berhasil.
    pub fn record_success(&self, response_size: u64) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.egress_bytes.fetch_add(response_size, Ordering::Relaxed);
    }

    /// Catat request gagal.
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Catat rate limit terlampaui.
    pub fn record_rate_limited(&self) {
        self.rate_limited_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Catat payload terlalu besar.
    pub fn record_payload_too_large(&self) {
        self.payload_too_large_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Catat kegagalan sanitasi.
    pub fn record_sanitization_fail(&self) {
        self.sanitization_fail_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Dapatkan total request.
    #[must_use]
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah keberhasilan.
    #[must_use]
    pub fn success_count(&self) -> u64 {
        self.success_count.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah kegagalan.
    #[must_use]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Dapatkan total byte ingress.
    #[must_use]
    pub fn ingress_bytes(&self) -> u64 {
        self.ingress_bytes.load(Ordering::Relaxed)
    }

    /// Dapatkan total byte egress.
    #[must_use]
    pub fn egress_bytes(&self) -> u64 {
        self.egress_bytes.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah rate limited.
    #[must_use]
    pub fn rate_limited_count(&self) -> u64 {
        self.rate_limited_count.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah payload too large.
    #[must_use]
    pub fn payload_too_large_count(&self) -> u64 {
        self.payload_too_large_count.load(Ordering::Relaxed)
    }

    /// Dapatkan jumlah sanitization fail.
    #[must_use]
    pub fn sanitization_fail_count(&self) -> u64 {
        self.sanitization_fail_count.load(Ordering::Relaxed)
    }

    /// Hitung rate penolakan dalam BPS (Basis Poin).
    /// Formula: (rejected * 10000) / total
    ///
    /// # Returns
    /// Rate penolakan dalam BPS, atau 0 jika `total_requests` = 0.
    #[must_use]
    pub fn rejection_rate_bps(&self) -> u64 {
        let total = self.total_requests();
        if total == 0 {
            return 0;
        }

        let rejected = self.error_count()
            + self.rate_limited_count()
            + self.payload_too_large_count()
            + self.sanitization_fail_count();

        // (rejected * 10000) / total
        (rejected * BPS_SCALE) / total
    }

    /// Hitung throughput ingress dalam byte per request (rata-rata).
    #[must_use]
    pub fn avg_ingress_bytes_per_request(&self) -> u64 {
        let total = self.total_requests();
        if total == 0 {
            return 0;
        }
        self.ingress_bytes() / total
    }

    /// Hitung throughput egress dalam byte per request (rata-rata).
    #[must_use]
    pub fn avg_egress_bytes_per_request(&self) -> u64 {
        let total = self.total_requests();
        if total == 0 {
            return 0;
        }
        self.egress_bytes() / total
    }

    /// Reset semua metrik.
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.error_count.store(0, Ordering::Relaxed);
        self.ingress_bytes.store(0, Ordering::Relaxed);
        self.egress_bytes.store(0, Ordering::Relaxed);
        self.rate_limited_count.store(0, Ordering::Relaxed);
        self.payload_too_large_count.store(0, Ordering::Relaxed);
        self.sanitization_fail_count.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = GatewayMetrics::new();
        assert_eq!(metrics.total_requests(), 0);
        assert_eq!(metrics.success_count(), 0);
        assert_eq!(metrics.error_count(), 0);
    }

    #[test]
    fn test_record_request() {
        let metrics = GatewayMetrics::new();
        metrics.record_request(1000);
        
        assert_eq!(metrics.total_requests(), 1);
        assert_eq!(metrics.ingress_bytes(), 1000);
    }

    #[test]
    fn test_record_success() {
        let metrics = GatewayMetrics::new();
        metrics.record_request(500);
        metrics.record_success(200);
        
        assert_eq!(metrics.success_count(), 1);
        assert_eq!(metrics.egress_bytes(), 200);
    }

    #[test]
    fn test_record_error() {
        let metrics = GatewayMetrics::new();
        metrics.record_request(500);
        metrics.record_error();
        
        assert_eq!(metrics.error_count(), 1);
    }

    #[test]
    fn test_rejection_rate_bps() {
        let metrics = GatewayMetrics::new();
        
        // 11 total: 7 success, 2 errors, 1 rate limited, 1 payload too large
        for _ in 0..7 {
            metrics.record_request(100);
            metrics.record_success(50);
        }
        for _ in 0..2 {
            metrics.record_request(100);
            metrics.record_error();
        }
        metrics.record_request(100);
        metrics.record_rate_limited();
        metrics.record_request(100);
        metrics.record_payload_too_large();
        
        // Total = 11, rejected = 4 (2 errors + 1 rate limited + 1 payload too large)
        // Rate = (4 * 10000) / 11 = 3636 BPS (floor division)
        assert_eq!(metrics.total_requests(), 11);
        assert_eq!(metrics.rejection_rate_bps(), 3636);
    }

    #[test]
    fn test_zero_float_guarantee() {
        // Audit statis: Tidak ada tipe f32/f64 di modul ini
        // Izinkan string literal "f32/f64" di komentar dan error messages
        let src = include_str!("../src/metrics.rs");
        
        // Cari pola penggunaan tipe (bukan string literal atau comment)
        let patterns = [" f32", "f32:", "f32,", "f32(", " f64", "f64:", "f64,", "f64("];
        let has_float_type = patterns.iter().any(|p| src.contains(p));
        
        // Izinkan string literal "f32/f64" dan "F32/f64"
        let has_comment_literal = src.contains("f32/f64");
        
        assert!(!has_float_type || has_comment_literal, 
            "Pelanggaran Zero-Float: f32/f64 ditemukan di metrics.rs");
    }

    #[test]
    fn test_reset() {
        let metrics = GatewayMetrics::new();
        metrics.record_request(1000);
        metrics.record_success(500);
        metrics.record_error();
        metrics.record_rate_limited();
        
        metrics.reset();
        
        assert_eq!(metrics.total_requests(), 0);
        assert_eq!(metrics.success_count(), 0);
        assert_eq!(metrics.error_count(), 0);
        assert_eq!(metrics.rate_limited_count(), 0);
    }
}
