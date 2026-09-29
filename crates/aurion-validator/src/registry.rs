use crate::{
    endorsement::{AdmissionPetition, PeerEndorsementCertificate},
    error::AdmissionError,
    probation::{Heartbeat, ProbationTracker, DEFAULT_PROBATION_BLOCKS},
};
use aurion_criptografi::PublicKeyBytes;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeAdmissionStatus {
    /// Sedang dalam masa uji coba 1 minggu
    InProbation(ProbationTracker),
    /// Telah lulus uji coba, sedang mengumpulkan 3 persetujuan validator
    Candidate(PeerEndorsementCertificate),
    /// Resmi aktif berpartisipasi dalam konsensus BFT
    Active,
    /// Gagal uji coba atau didiskualifikasi
    Disqualified,
}

#[derive(Debug)]
pub struct ValidatorRegistry {
    pub nodes: BTreeMap<PublicKeyBytes, NodeAdmissionStatus>,
    pub active_validators: BTreeSet<PublicKeyBytes>,
    pub probation_duration: u64,
}

impl ValidatorRegistry {
    pub fn new(initial_validators: Vec<PublicKeyBytes>, probation_duration: u64) -> Self {
        let mut nodes = BTreeMap::new();
        let mut active_validators = BTreeSet::new();

        for val in initial_validators {
            nodes.insert(val, NodeAdmissionStatus::Active);
            active_validators.insert(val);
        }

        Self {
            nodes,
            active_validators,
            probation_duration: if probation_duration == 0 {
                DEFAULT_PROBATION_BLOCKS
            } else {
                probation_duration
            },
        }
    }

    /// 1. Pendaftaran baru ke masa uji coba
    pub fn register_for_probation(
        &mut self,
        node: PublicKeyBytes,
        current_block: u64,
    ) -> Result<(), AdmissionError> {
        if self.nodes.contains_key(&node) {
            return Err(AdmissionError::AlreadyRegistered(node));
        }

        let tracker = ProbationTracker::new(current_block, self.probation_duration);
        self.nodes
            .insert(node, NodeAdmissionStatus::InProbation(tracker));
        tracing::info!(validator = ?node, current_block, "Validator baru masuk masa probation");
        Ok(())
    }

    /// 2. Proses Heartbeat selama masa probation
    pub fn handle_heartbeat(
        &mut self,
        heartbeat: &Heartbeat,
        current_block: u64,
    ) -> Result<(), AdmissionError> {
        heartbeat.verify()?;

        let status = self
            .nodes
            .get_mut(&heartbeat.node_key)
            .ok_or(AdmissionError::NodeNotFound(heartbeat.node_key))?;

        if let NodeAdmissionStatus::InProbation(tracker) = status {
            tracker.record_heartbeat(current_block);
            Ok(())
        } else {
            Ok(())
        }
    }

    /// 3. Luluskan Probation -> Promosikan ke Status Candidate
    pub fn graduate_to_candidate(
        &mut self,
        node: &PublicKeyBytes,
        current_block: u64,
    ) -> Result<(), AdmissionError> {
        let status = self
            .nodes
            .get_mut(node)
            .ok_or(AdmissionError::NodeNotFound(*node))?;

        let uptime = match status {
            NodeAdmissionStatus::InProbation(tracker) => {
                tracker.evaluate_completion(current_block)?;
                tracker.uptime_percentage()
            }
            _ => return Err(AdmissionError::NotEligibleForCandidacy),
        };

        let petition = AdmissionPetition {
            candidate: *node,
            probation_end_block: current_block,
            uptime_percentage: uptime,
        };

        *status = NodeAdmissionStatus::Candidate(PeerEndorsementCertificate::new(petition));
        tracing::info!(validator = ?node, uptime_percent = uptime, "Validator lulus probation dan menjadi kandidat");
        Ok(())
    }

    /// 4. Berikan suara dukungan (Endorsement) dari validator aktif
    pub fn submit_endorsement(
        &mut self,
        candidate: &PublicKeyBytes,
        endorser: PublicKeyBytes,
        signature: [u8; 64],
    ) -> Result<bool, AdmissionError> {
        let is_active = self.active_validators.contains(&endorser);

        let status = self
            .nodes
            .get_mut(candidate)
            .ok_or(AdmissionError::NodeNotFound(*candidate))?;

        let mut should_promote = false;

        if let NodeAdmissionStatus::Candidate(cert) = status {
            cert.add_endorsement(endorser, signature, is_active)?;

            // Jika sudah mencapai 3 persetujuan, simpul siap dipromosikan ke Active
            if cert.verify_threshold().is_ok() {
                should_promote = true;
            }
        } else {
            return Err(AdmissionError::NotEligibleForCandidacy);
        }

        if should_promote {
            *status = NodeAdmissionStatus::Active;
            self.active_validators.insert(*candidate);
            tracing::info!(validator = ?candidate, "Validator resmi dipromosikan ke active set");
            return Ok(true); // Resmi menjadi Validator Aktif
        }

        Ok(false) // Masih menunggu persetujuan berikutnya
    }
}
