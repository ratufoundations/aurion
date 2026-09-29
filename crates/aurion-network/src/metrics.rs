//! Metrik jaringan integer murni (`u64` milidetik / bita): timeout handshake,
//! interval keepalive, pelacakan RTT, dan akuntansi bandwidth anti-overflow.
//! Seluruh perhitungan zero-float agar deterministik lintas arsitektur.

/// Batas waktu handshake sebelum koneksi ditutup (milidetik).
pub const HANDSHAKE_TIMEOUT_MS: u64 = 5_000;
/// Interval ping keepalive antar-peer (milidetik).
pub const KEEPALIVE_PING_INTERVAL_MS: u64 = 1_000;
/// Batas atas RTT yang masih dianggap waras untuk pelacakan (milidetik).
pub const MAX_RTT_MS: u64 = 30_000;

/// Akuntansi volume transfer dua arah dengan proteksi `checked_add`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BandwidthMeter {
    sent_bytes: u64,
    received_bytes: u64,
}

impl BandwidthMeter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sent_bytes: 0,
            received_bytes: 0,
        }
    }

    #[must_use]
    pub const fn sent_bytes(&self) -> u64 {
        self.sent_bytes
    }

    #[must_use]
    pub const fn received_bytes(&self) -> u64 {
        self.received_bytes
    }

    /// Catat bita keluar; mengembalikan `false` tanpa mutasi bila overflow.
    pub fn record_sent(&mut self, delta: u64) -> bool {
        match self.sent_bytes.checked_add(delta) {
            Some(next) => {
                self.sent_bytes = next;
                true
            }
            None => false,
        }
    }

    /// Catat bita masuk; mengembalikan `false` tanpa mutasi bila overflow.
    pub fn record_received(&mut self, delta: u64) -> bool {
        match self.received_bytes.checked_add(delta) {
            Some(next) => {
                self.received_bytes = next;
                true
            }
            None => false,
        }
    }

    /// Total bita dua arah; `None` bila penjumlahannya overflow.
    #[must_use]
    pub fn total_bytes(&self) -> Option<u64> {
        self.sent_bytes.checked_add(self.received_bytes)
    }
}

/// Pelacak RTT integer: exponential moving average `EMA = (3*EMA + sampel) / 4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RttTracker {
    ema_ms: u64,
}

impl RttTracker {
    #[must_use]
    pub const fn new(initial_ms: u64) -> Self {
        Self { ema_ms: initial_ms }
    }

    #[must_use]
    pub const fn ema_ms(&self) -> u64 {
        self.ema_ms
    }

    /// Amati satu sampel RTT; mengembalikan `None` tanpa mutasi bila overflow.
    pub fn observe(&mut self, sample_ms: u64) -> Option<u64> {
        let scaled = self.ema_ms.checked_mul(3)?.checked_add(sample_ms)?;
        self.ema_ms = scaled / 4;
        Some(self.ema_ms)
    }
}

/// Deadline handshake absolut dari jam mulai; `None` bila overflow.
#[must_use]
pub fn handshake_deadline_ms(start_ms: u64, timeout_ms: u64) -> Option<u64> {
    start_ms.checked_add(timeout_ms)
}
