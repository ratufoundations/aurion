//! Ledger eksekusi bukti untuk pencegahan replay.
//!
//! Modul ini menyediakan mekanisme untuk mencatat bukti-bukti yang sudah
//! dieksekusi agar tidak dapat diproses ulang.

use aurion_criptografi::Hash256;
use std::collections::BTreeSet;

/// Usia maksimal bukti dalam blok (misal: 10.000 blok = ~3 jam pada 1 blok/3 detik)
pub const MAX_EVIDENCE_AGE_BLOCKS: u64 = 10_000;

/// Ledger pencatatan bukti yang sudah dieksekusi.
/// Menggunakan `BTreeSet` untuk pengurutan deterministik.
#[derive(Debug, Clone, Default)]
pub struct ExecutedEvidenceLedger {
    executed_digests: BTreeSet<Hash256>,
}

impl ExecutedEvidenceLedger {
    /// Buat ledger baru.
    #[must_use]
    pub fn new() -> Self {
        Self {
            executed_digests: BTreeSet::new(),
        }
    }

    /// Periksa apakah bukti sudah pernah dieksekusi.
    #[must_use]
    pub fn is_executed(&self, evidence_digest: &Hash256) -> bool {
        self.executed_digests.contains(evidence_digest)
    }

    /// Catat bukti sebagai sudah dieksekusi.
    ///
    /// # Arguments
    /// * `evidence_digest` - Hash BLAKE3 dari bukti
    ///
    /// # Returns
    /// `true` jika bukti sudah ada (tidak ada perubahan),
    /// `false` jika bukti baru dan berhasil dicatat.
    pub fn record_execution(&mut self, evidence_digest: Hash256) -> bool {
        !self.executed_digests.insert(evidence_digest)
    }

    /// Dapatkan jumlah bukti yang sudah dieksekusi.
    #[must_use]
    pub fn len(&self) -> usize {
        self.executed_digests.len()
    }

    /// Periksa apakah ledger kosong.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.executed_digests.is_empty()
    }

    /// Kosongkan ledger (untuk testing saja).
    pub fn clear(&mut self) {
        self.executed_digests.clear();
    }
}

/// Informasi kontekstual untuk validasi kedaluwarsa bukti.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceContext {
    /// Tinggi blok saat bukti dibuat.
    pub evidence_block_height: u64,
    /// Tinggi blok saat ini (saat validasi).
    pub current_block_height: u64,
}

impl EvidenceContext {
    /// Periksa apakah bukti masih valid (tidak kedaluwarsa).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.current_block_height <= self.evidence_block_height + MAX_EVIDENCE_AGE_BLOCKS
    }

    /// Hitung usia bukti dalam blok.
    #[must_use]
    pub fn age_blocks(&self) -> u64 {
        self.current_block_height
            .saturating_sub(self.evidence_block_height)
    }

    /// Dapatkan usia maksimal yang diperbolehkan.
    #[must_use]
    pub fn max_age(&self) -> u64 {
        MAX_EVIDENCE_AGE_BLOCKS
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_ledger_basic() {
        let mut ledger = ExecutedEvidenceLedger::new();
        assert_eq!(ledger.len(), 0);

        let digest1 = [1u8; 32];
        let digest2 = [2u8; 32];

        // Catat bukti pertama
        assert!(!ledger.is_executed(&digest1));
        assert!(!ledger.record_execution(digest1));
        assert!(ledger.is_executed(&digest1));
        assert_eq!(ledger.len(), 1);

        // Catat bukti kedua
        assert!(!ledger.is_executed(&digest2));
        assert!(!ledger.record_execution(digest2));
        assert!(ledger.is_executed(&digest2));
        assert_eq!(ledger.len(), 2);

        // Coba catat ulang bukti pertama
        assert!(ledger.record_execution(digest1));
        assert_eq!(ledger.len(), 2); // Tidak bertambah
    }

    #[test]
    fn test_evidence_context_validity() {
        let ctx_fresh = EvidenceContext {
            evidence_block_height: 100_000,
            current_block_height: 100_000,
        };
        assert!(ctx_fresh.is_valid());
        assert_eq!(ctx_fresh.age_blocks(), 0);

        let ctx_at_limit = EvidenceContext {
            evidence_block_height: 100_000,
            current_block_height: 100_000 + MAX_EVIDENCE_AGE_BLOCKS,
        };
        assert!(ctx_at_limit.is_valid());
        assert_eq!(ctx_at_limit.age_blocks(), MAX_EVIDENCE_AGE_BLOCKS);

        let ctx_expired = EvidenceContext {
            evidence_block_height: 100_000,
            current_block_height: 100_000 + MAX_EVIDENCE_AGE_BLOCKS + 1,
        };
        assert!(!ctx_expired.is_valid());
        assert_eq!(
            ctx_expired.age_blocks(),
            MAX_EVIDENCE_AGE_BLOCKS + 1
        );
    }
}
