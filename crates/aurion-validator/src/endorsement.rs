use crate::error::AdmissionError;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};
use std::collections::BTreeMap;

pub const MINIMUM_PEER_APPROVALS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionPetition {
    pub candidate: PublicKeyBytes,
    pub probation_end_block: u64,
    pub uptime_percentage: u64,
}

impl AdmissionPetition {
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_VALIDATOR_ADMISSION_PETITION_V1");
        hasher.update(&self.candidate);
        hasher.update(&self.probation_end_block.to_le_bytes());
        hasher.update(&self.uptime_percentage.to_le_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerEndorsementCertificate {
    pub petition: AdmissionPetition,
    /// Daftar tanda tangan dukungan: Endorser Pubkey -> Signature
    pub endorsements: BTreeMap<PublicKeyBytes, SignatureBytes>,
}

impl PeerEndorsementCertificate {
    pub fn new(petition: AdmissionPetition) -> Self {
        Self {
            petition,
            endorsements: BTreeMap::new(),
        }
    }

    /// Tambah suara dukungan dari validator aktif
    pub fn add_endorsement(
        &mut self,
        endorser: PublicKeyBytes,
        signature: SignatureBytes,
        is_active_validator: bool,
    ) -> Result<(), AdmissionError> {
        if endorser == self.petition.candidate {
            return Err(AdmissionError::SelfEndorsementForbidden);
        }

        if !is_active_validator {
            return Err(AdmissionError::UnauthorizedEndorser(endorser));
        }

        if self.endorsements.contains_key(&endorser) {
            return Err(AdmissionError::DuplicateEndorsement(endorser));
        }

        // Verifikasi keaslian tanda tangan atas petisi
        let digest = self.petition.digest();
        SignatureVerifier::verify_single(&endorser, &digest, &signature)
            .map_err(|_| AdmissionError::InvalidSignature)?;

        self.endorsements.insert(endorser, signature);
        Ok(())
    }

    /// Verifikasi apakah ambang batas minimal 3 validator telah terpenuhi
    pub fn verify_threshold(&self) -> Result<(), AdmissionError> {
        if self.endorsements.len() < MINIMUM_PEER_APPROVALS {
            return Err(AdmissionError::InsufficientEndorsements {
                collected: self.endorsements.len(),
                required: MINIMUM_PEER_APPROVALS,
            });
        }
        Ok(())
    }
}
