use aurion_criptografi::{Hash256, PublicKeyBytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationType {
    /// Menandatangani 2 blok berbeda pada ketinggian yang sama (Double Signing)
    DoubleSigning,
    /// Memproduksi blok dengan State Root yang tidak deterministik/palsu
    InvalidStateRootProposal,
    /// Mengirimkan frame biner korup secara terus menerus (DDoS/Spam)
    NetworkSabotage,
    /// Simpul mati berkepanjangan tanpa konfirmasi saat gilirannya tiba
    UnresponsiveLivenessFailure,
}

impl ViolationType {
    /// `true` bila pelanggaran ini bermuatan penipuan akut yang wajib diikuti
    /// karantina ireversibel validator (`Tombstoned`), bukan sekadar hukuman.
    ///
    /// Kebijakan saat ini konservatif: hanya `DoubleSigning` yang memicu
    /// tombstone. Peta dapat diperluas bila mekanisme slashing dan karantina
    /// stake untuk pelanggaran berat lain sudah berdampingan di orkestrator.
    #[must_use]
    pub const fn requires_tombstone(&self) -> bool {
        matches!(self, Self::DoubleSigning)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaidEvidence {
    pub target_validator: PublicKeyBytes,
    pub violation: ViolationType,
    pub block_height: u64,
    pub proof_hash: Hash256,
}

impl RaidEvidence {
    #[must_use]
    pub fn new(
        target_validator: PublicKeyBytes,
        violation: ViolationType,
        block_height: u64,
        proof_payload: &[u8],
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_RAID_EVIDENCE_V1");
        hasher.update(&target_validator);
        hasher.update(&block_height.to_le_bytes());
        hasher.update(proof_payload);
        let proof_hash = *hasher.finalize().as_bytes();

        Self {
            target_validator,
            violation,
            block_height,
            proof_hash,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehabilitationPetition {
    pub target_validator: PublicKeyBytes,
    pub justification_hash: Hash256,
    pub timestamp: u64,
}

impl RehabilitationPetition {
    #[must_use]
    pub fn new(target_validator: PublicKeyBytes, explanation: &str, timestamp: u64) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_REHABILITATION_PETITION_V1");
        hasher.update(&target_validator);
        hasher.update(explanation.as_bytes());
        hasher.update(&timestamp.to_le_bytes());
        let justification_hash = *hasher.finalize().as_bytes();

        Self {
            target_validator,
            justification_hash,
            timestamp,
        }
    }
}
