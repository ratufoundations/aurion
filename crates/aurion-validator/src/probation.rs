use crate::error::AdmissionError;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

/// Asumsi 2 detik per blok: 1 Minggu = 7 * 86.400 / 2 = 302.400 Blok
pub const DEFAULT_PROBATION_BLOCKS: u64 = 302_400;
pub const MINIMUM_UPTIME_PERCENT: u64 = 99; // Wajib online minimal 99%

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heartbeat {
    pub node_key: PublicKeyBytes,
    pub block_height: u64,
    pub signature: SignatureBytes,
}

impl Heartbeat {
    #[must_use]
    pub fn digest(node_key: &PublicKeyBytes, block_height: u64) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_VALIDATOR_HEARTBEAT_V1");
        hasher.update(node_key);
        hasher.update(&block_height.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Memeriksa tanda tangan heartbeat menggunakan kunci simpul.
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika tanda tangan tidak valid.
    pub fn verify(&self) -> Result<(), AdmissionError> {
        let digest = Self::digest(&self.node_key, self.block_height);
        SignatureVerifier::verify_single(&self.node_key, &digest, &self.signature)
            .map_err(|_| AdmissionError::InvalidSignature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbationTracker {
    pub start_block: u64,
    pub probation_duration_blocks: u64,
    pub heartbeats_received: u64,
    pub expected_heartbeats: u64,
    pub last_heartbeat_block: u64,
}

impl ProbationTracker {
    #[must_use]
    pub fn new(start_block: u64, duration_blocks: u64) -> Self {
        Self {
            start_block,
            probation_duration_blocks: duration_blocks,
            heartbeats_received: 0,
            expected_heartbeats: 0,
            last_heartbeat_block: start_block,
        }
    }

    /// Catat heartbeat yang masuk
    pub fn record_heartbeat(&mut self, current_block: u64) {
        self.heartbeats_received = self.heartbeats_received.saturating_add(1);
        self.last_heartbeat_block = current_block;
    }

    /// Naikkan ekspektasi heartbeat pada interval pemeriksaan
    pub fn tick_expected(&mut self) {
        self.expected_heartbeats = self.expected_heartbeats.saturating_add(1);
    }

    /// Hitung rasio keaktifan dalam persentase integer murni (0 - 100)
    #[must_use]
    pub fn uptime_percentage(&self) -> u64 {
        if self.expected_heartbeats == 0 {
            return 100;
        }
        (self.heartbeats_received.saturating_mul(100)) / self.expected_heartbeats
    }

    /// Evaluasi apakah simpul lulus masa uji coba 1 minggu
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika masa uji coba belum selesai atau uptime di bawah
    /// persyaratan minimum.
    pub fn evaluate_completion(&self, current_block: u64) -> Result<(), AdmissionError> {
        let elapsed = current_block.saturating_sub(self.start_block);
        if elapsed < self.probation_duration_blocks {
            return Err(AdmissionError::ProbationNotFinished {
                elapsed,
                required: self.probation_duration_blocks,
            });
        }

        let uptime = self.uptime_percentage();
        if uptime < MINIMUM_UPTIME_PERCENT {
            return Err(AdmissionError::InsufficientUptime {
                actual: uptime,
                required: MINIMUM_UPTIME_PERCENT,
            });
        }

        Ok(())
    }
}
