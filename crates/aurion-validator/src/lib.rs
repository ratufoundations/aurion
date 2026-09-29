#![forbid(unsafe_code)]

pub mod endorsement;
pub mod error;
pub mod probation;
pub mod registry;

pub use endorsement::{AdmissionPetition, PeerEndorsementCertificate, MINIMUM_PEER_APPROVALS};
pub use error::AdmissionError;
pub use probation::{
    Heartbeat, ProbationTracker, DEFAULT_PROBATION_BLOCKS, MINIMUM_UPTIME_PERCENT,
};
pub use registry::{NodeAdmissionStatus, ValidatorRegistry};

#[cfg(test)]
mod tests {
    use super::*;
    use aurion_criptografi::Keypair;

    #[test]
    fn test_full_validator_admission_lifecycle() {
        // 1. Inisialisasi jaringan dengan 3 validator awal
        let v1 = Keypair::generate();
        let v2 = Keypair::generate();
        let v3 = Keypair::generate();

        let initial_vals = vec![
            v1.public_key_bytes(),
            v2.public_key_bytes(),
            v3.public_key_bytes(),
        ];

        // Buat uji coba dengan durasi 100 blok (simulasi 1 minggu)
        let trial_duration_blocks = 100u64;
        let mut registry = ValidatorRegistry::new(initial_vals, trial_duration_blocks);

        // 2. Calon node mendaftar masa uji coba
        let candidate_keypair = Keypair::generate();
        let candidate_pk = candidate_keypair.public_key_bytes();
        registry
            .register_for_probation(candidate_pk, 0)
            .expect("test operation should succeed");

        // 3. Simulasi pengiriman Heartbeat selama masa probation
        for block in 1..=100 {
            if let Some(NodeAdmissionStatus::InProbation(t)) = registry.nodes.get_mut(&candidate_pk)
            {
                t.tick_expected();
            }

            let hb_digest = Heartbeat::digest(&candidate_pk, block);
            let hb = Heartbeat {
                node_key: candidate_pk,
                block_height: block,
                signature: candidate_keypair.sign(&hb_digest),
            };
            registry
                .handle_heartbeat(&hb, block)
                .expect("test operation should succeed");
        }

        // Coba luluskan sebelum blok 100 -> GAGAL
        assert!(registry.graduate_to_candidate(&candidate_pk, 99).is_err());

        // Luluskan pada blok 100 dengan uptime 100% -> SUKSES MASUK KANDIDAT
        registry
            .graduate_to_candidate(&candidate_pk, 100)
            .expect("test operation should succeed");

        // 4. Koleksi 3 Persetujuan Validator Aktif
        let petition_digest = match registry
            .nodes
            .get(&candidate_pk)
            .expect("test operation should succeed")
        {
            NodeAdmissionStatus::Candidate(cert) => cert.petition.digest(),
            _ => panic!("Status harus Candidate"),
        };

        // Validator 1 menyetujui
        let sig1 = v1.sign(&petition_digest);
        let promoted = registry
            .submit_endorsement(&candidate_pk, v1.public_key_bytes(), sig1)
            .expect("test operation should succeed");
        assert!(!promoted); // Baru 1/3

        // Validator 2 menyetujui
        let sig2 = v2.sign(&petition_digest);
        let promoted = registry
            .submit_endorsement(&candidate_pk, v2.public_key_bytes(), sig2)
            .expect("test operation should succeed");
        assert!(!promoted); // Baru 2/3

        // Calon mencoba menyetujui dirinya sendiri -> DITOLAK
        let self_sig = candidate_keypair.sign(&petition_digest);
        assert_eq!(
            registry
                .submit_endorsement(&candidate_pk, candidate_pk, self_sig)
                .unwrap_err(),
            AdmissionError::SelfEndorsementForbidden
        );

        // Validator 3 menyetujui (Genap 3/3 validator aktif)
        let sig3 = v3.sign(&petition_digest);
        let promoted = registry
            .submit_endorsement(&candidate_pk, v3.public_key_bytes(), sig3)
            .expect("test operation should succeed");
        assert!(promoted); // RESMI AKTIF

        // 5. Verifikasi status akhir
        assert_eq!(
            registry.nodes.get(&candidate_pk),
            Some(&NodeAdmissionStatus::Active)
        );
        assert!(registry.active_validators.contains(&candidate_pk));
        assert_eq!(registry.active_validators.len(), 4);
    }
}
