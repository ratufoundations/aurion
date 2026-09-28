use crate::evidence::{RaidEvidence, RehabilitationPetition};
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes};
use std::collections::BTreeMap;

/// Sertifikat Pemutusan Jaringan (Blacklist) - Wajib 5/5 Suara Guard
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlacklistVerdict {
    pub evidence: RaidEvidence,
    pub signatures: BTreeMap<PublicKeyBytes, SignatureBytes>,
}

impl BlacklistVerdict {
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_BLACKLIST_VERDICT_V1");
        hasher.update(&self.evidence.target_validator);
        hasher.update(&self.evidence.block_height.to_le_bytes());
        hasher.update(&self.evidence.proof_hash);
        *hasher.finalize().as_bytes()
    }
}

/// Sertifikat Pemulihan & Hapus Blacklist (Pardon) - Wajib 5/5 Suara Guard
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PardonVerdict {
    pub petition: RehabilitationPetition,
    pub signatures: BTreeMap<PublicKeyBytes, SignatureBytes>,
}

impl PardonVerdict {
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_PARDON_VERDICT_V1");
        hasher.update(&self.petition.target_validator);
        hasher.update(&self.petition.justification_hash);
        hasher.update(&self.petition.timestamp.to_le_bytes());
        *hasher.finalize().as_bytes()
    }
}
