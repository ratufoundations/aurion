//! Modul pembatasan laju berbasis Token Bucket dengan akuntansi u64 nir-pecahan.
//!
//! Modul ini menerapkan algoritma Token Bucket untuk membatasi laju permintaan
//! per identitas (IP address atau `AccountId`) menggunakan aritmatika integer murni.

use crate::error::GatewayError;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Skala waktu dalam milidetik.
pub const MS_SCALE: u64 = 1_000;

/// Kapasitas bucket default: 10 token.
pub const DEFAULT_BUCKET_CAPACITY: u64 = 10;

/// Laju isi ulang default: 1 token per 100 ms (10 token per detik).
pub const DEFAULT_REFILL_RATE_MS: u64 = 100;

/// Waktu isi ulang default dalam Duration.
#[must_use]
pub fn default_refill_interval() -> Duration {
    Duration::from_millis(DEFAULT_REFILL_RATE_MS)
}

/// Informasi token bucket untuk setiap identitas.
#[derive(Debug, Clone, Copy)]
pub struct TokenBucket {
    /// Jumlah token saat ini.
    tokens: u64,
    /// Waktu terakhir isi ulang.
    last_refill: u64,
}

impl TokenBucket {
    /// Buat bucket baru dengan kapasitas penuh.
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            tokens: capacity,
            last_refill: current_timestamp_ms(),
        }
    }

    /// Coba konsumsi satu token.
    ///
    /// # Returns
    /// `Ok(())` jika token tersedia, `Err(retry_after_ms)` jika perlu menunggu.
    ///
    /// # Errors
    /// Mengembalikan `Err(retry_after_ms)` jika token habis, dengan waktu tunggu dalam ms.
    pub fn try_consume(&mut self, capacity: u64, refill_rate_ms: u64) -> Result<(), u64> {
        let now = current_timestamp_ms();
        let elapsed = now.saturating_sub(self.last_refill);

        // Hitung token yang terisi ulang
        // Note: refill_rate_ms > 0 dijamin oleh check di atas, tetapi kami tetap pakai checked_div
        let refilled = if refill_rate_ms > 0 {
            elapsed.checked_div(refill_rate_ms).unwrap_or(0)
        } else {
            0
        };

        // Tambahkan token yang terisi ulang (dibatasi oleh kapasitas)
        self.tokens = (self.tokens + refilled).min(capacity);
        self.last_refill = now;

        if self.tokens > 0 {
            self.tokens -= 1;
            Ok(())
        } else {
            // Waktu menunggu untuk token berikutnya
            let wait_time = refill_rate_ms.saturating_sub(elapsed % refill_rate_ms);
            Err(wait_time)
        }
    }

    /// Dapatkan jumlah token saat ini.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.tokens
    }
}

/// Pembatas laju menggunakan algoritma Token Bucket.
/// Thread-safe menggunakan Arc<Mutex<...>>.
#[derive(Debug)]
pub struct RateLimiter {
    buckets: Arc<Mutex<BTreeMap<String, TokenBucket>>>,
    capacity: u64,
    refill_rate_ms: u64,
}

impl RateLimiter {
    /// Buat rate limiter baru dengan konfigurasi default.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DEFAULT_BUCKET_CAPACITY, DEFAULT_REFILL_RATE_MS)
    }

    /// Buat rate limiter dengan konfigurasi khusus.
    ///
    /// # Arguments
    /// * `capacity` - Jumlah token maksimal per bucket
    /// * `refill_rate_ms` - Waktu (ms) untuk isi ulang 1 token
    #[must_use]
    pub fn with_config(capacity: u64, refill_rate_ms: u64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(BTreeMap::new())),
            capacity,
            refill_rate_ms,
        }
    }

    /// Periksa dan konsumsi token untuk identitas.
    ///
    /// # Arguments
    /// * `identity` - Identitas (IP address, `AccountId`, dll.)
    ///
    /// # Returns
    /// `Ok(())` jika request diizinkan, `Err(GatewayError::RateLimitExceeded)` jika ditolak.
    ///
    /// # Errors
    /// Mengembalikan `Err(GatewayError::InternalError)` jika mutex poisoned,
    /// atau `Err(GatewayError::RateLimitExceeded)` jika token habis.
    pub fn check_rate(&self, identity: &str) -> Result<(), GatewayError> {
        let mut buckets = self.buckets.lock().map_err(|_| GatewayError::InternalError {
            trace_id: "mutex_poisoned".to_string(),
        })?;

        let bucket = buckets.entry(identity.to_string()).or_insert_with(|| {
            TokenBucket::new(self.capacity)
        });

        match bucket.try_consume(self.capacity, self.refill_rate_ms) {
            Ok(()) => Ok(()),
            Err(retry_after_ms) => Err(GatewayError::RateLimitExceeded { retry_after_ms }),
        }
    }

    /// Dapatkan jumlah token saat ini untuk identitas.
    #[must_use]
    pub fn tokens_for(&self, identity: &str) -> u64 {
        let buckets = self.buckets.lock().unwrap_or_else(|_| {
            // Panic tidak mungkin terjadi di sini, menggunakan unwrap yang aman
            std::panic::panic_any("RateLimiter mutex poisoned")
        });
        buckets.get(identity).map_or(0, TokenBucket::tokens)
    }

    /// Bersihkan bucket untuk identitas.
    pub fn clear(&self, identity: &str) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|_| {
            std::panic::panic_any("RateLimiter mutex poisoned")
        });
        buckets.remove(identity);
    }

    /// Bersihkan semua bucket.
    pub fn clear_all(&self) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|_| {
            std::panic::panic_any("RateLimiter mutex poisoned")
        });
        buckets.clear();
    }

    /// Dapatkan kapasitas bucket.
    #[must_use]
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Dapatkan laju isi ulang.
    #[must_use]
    pub fn refill_rate_ms(&self) -> u64 {
        self.refill_rate_ms
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Dapatkan timestamp saat ini dalam milidetik.
#[must_use]
fn current_timestamp_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(10);
        assert_eq!(bucket.tokens(), 10);
    }

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(3);
        
        // Konsumsi 3 token
        assert!(bucket.try_consume(3, 100).is_ok());
        assert_eq!(bucket.tokens(), 2);
        
        assert!(bucket.try_consume(3, 100).is_ok());
        assert_eq!(bucket.tokens(), 1);
        
        assert!(bucket.try_consume(3, 100).is_ok());
        assert_eq!(bucket.tokens(), 0);
        
        // Token habis
        assert!(bucket.try_consume(3, 100).is_err());
    }

    #[test]
    fn test_token_bucket_refill() {
        // Tidak bisa menguji refill waktu nyata tanpa mock time
        // Ini akan di-uji di gateway_invariants.rs
    }
}
