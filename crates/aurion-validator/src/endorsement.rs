use crate::error::{AdmissionError, ValidatorError};
use aurion_account::AccountId;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};
use std::collections::{BTreeMap, BTreeSet};

pub const MINIMUM_PEER_APPROVALS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionPetition {
    pub candidate: PublicKeyBytes,
    pub probation_end_block: u64,
    pub uptime_percentage: u64,
}

impl AdmissionPetition {
    #[must_use]
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
    #[must_use]
    pub fn new(petition: AdmissionPetition) -> Self {
        Self {
            petition,
            endorsements: BTreeMap::new(),
        }
    }

    /// Tambah suara dukungan dari validator aktif
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika endorser adalah kandidat, tidak aktif, mengirim
    /// dukungan duplikat, atau tanda tangannya tidak valid.
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
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika jumlah dukungan belum mencapai ambang minimum.
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

/// Kuota maksimum endorsement aktif per validator pengesah (anti-kartel Sybil).
pub const MAX_ACTIVE_ENDORSEMENTS: usize = 3;

/// Masa kerja (*tenure*) minimum sebelum validator aktif boleh mengesahkan kandidat.
pub const MIN_ENDORSER_TENURE_BLOCKS: u64 = 1_200;

/// Satu pengesahan kandidat oleh validator aktif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endorsement {
    /// Validator aktif yang memberikan pengesahan.
    pub endorser: AccountId,
    /// Kandidat penerima pengesahan.
    pub candidate: AccountId,
    /// Blok saat pengesahan diberikan.
    pub granted_at_block: u64,
}

/// Buku besar endorsement berkuota dengan pencabutan (*taint*) otomatis.
///
/// Kuota per pengesah membatasi monopoli aliansi, sedangkan bobot efektif
/// dihitung tanpa rekurssi dan dengan potongan untuk pasangan timbal balik,
/// sehingga cincin validator saling dukung tidak dapat melipatgandakan bobot
/// secara artifisial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndorsementLedger {
    max_active_per_endorser: usize,
    entries: BTreeMap<(AccountId, AccountId), Endorsement>,
    tainted: BTreeSet<AccountId>,
}

impl EndorsementLedger {
    /// Bangun buku besar dengan kuota per pengesah.
    #[must_use]
    pub fn new(max_active_per_endorser: usize) -> Self {
        Self {
            max_active_per_endorser,
            entries: BTreeMap::new(),
            tainted: BTreeSet::new(),
        }
    }

    /// Kuota endorsement aktif per pengesah.
    #[must_use]
    pub const fn max_active_per_endorser(&self) -> usize {
        self.max_active_per_endorser
    }

    /// Jumlah endorsement aktif yang diterbitkan seorang pengesah.
    #[must_use]
    pub fn active_count(&self, endorser: &AccountId) -> usize {
        self.entries
            .keys()
            .filter(|(key, _)| key == endorser)
            .count()
    }

    /// `true` bila pengesah sudah pernah dicabut kewenangannya (`tainted`).
    #[must_use]
    pub fn is_tainted(&self, endorser: &AccountId) -> bool {
        self.tainted.contains(endorser)
    }

    /// `true` bila pengesahan tertentu masih berlaku.
    #[must_use]
    pub fn contains(&self, endorser: &AccountId, candidate: &AccountId) -> bool {
        self.entries.contains_key(&(*endorser, *candidate))
    }

    /// Terbitkan endorsement baru setelah pemeriksaan kuota dan duplikasi.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::EndorserTainted` bila pengesah sudah
    /// dicabut, `ValidatorError::SelfEndorsementForbidden` bila pengesah dan
    /// kandidat identik, `ValidatorError::DuplicateEndorsement` bila pengesahan
    /// sudah ada, dan `ValidatorError::EndorsementQuotaExceeded` bila kuota habis.
    pub fn grant(
        &mut self,
        endorser: &AccountId,
        candidate: &AccountId,
        granted_at_block: u64,
    ) -> Result<(), ValidatorError> {
        if self.tainted.contains(endorser) {
            return Err(ValidatorError::EndorserTainted(*endorser));
        }
        if endorser == candidate {
            return Err(ValidatorError::SelfEndorsementForbidden);
        }
        let key = (*endorser, *candidate);
        if self.entries.contains_key(&key) {
            return Err(ValidatorError::DuplicateEndorsement {
                endorser: *endorser,
                candidate: *candidate,
            });
        }
        let active = self.active_count(endorser);
        if active >= self.max_active_per_endorser {
            return Err(ValidatorError::EndorsementQuotaExceeded {
                active,
                max: self.max_active_per_endorser,
            });
        }
        self.entries.insert(
            key,
            Endorsement {
                endorser: *endorser,
                candidate: *candidate,
                granted_at_block,
            },
        );
        Ok(())
    }

    /// Cabut satu endorsement yang masih berlaku.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::EndorsementNotFound` bila pengesahan tidak ada.
    pub fn revoke(
        &mut self,
        endorser: &AccountId,
        candidate: &AccountId,
    ) -> Result<(), ValidatorError> {
        if self.entries.remove(&(*endorser, *candidate)).is_none() {
            return Err(ValidatorError::EndorsementNotFound {
                endorser: *endorser,
                candidate: *candidate,
            });
        }
        Ok(())
    }

    /// Tandai pengesah sebagai `tainted` dan cabut seluruh endorsement-nya.
    ///
    /// Dipicu penalti slashing berat atau putusan dewan; mengembalikan jumlah
    /// endorsement yang dicabut.
    pub fn taint_endorser(&mut self, endorser: &AccountId) -> usize {
        self.tainted.insert(*endorser);
        let keys: Vec<(AccountId, AccountId)> = self
            .entries
            .keys()
            .filter(|(key, _)| key == endorser)
            .copied()
            .collect();
        for key in &keys {
            self.entries.remove(key);
        }
        keys.len()
    }

    /// Daftar pengesah aktif (bebas taint) untuk seorang kandidat.
    #[must_use]
    pub fn endorsers_of(&self, candidate: &AccountId) -> Vec<AccountId> {
        self.entries
            .iter()
            .filter(|((_, target), _)| target == candidate)
            .map(|((endorser, _), _)| *endorser)
            .collect()
    }

    /// Bobot endorsement efektif dengan potongan hubungan timbal balik.
    ///
    /// Bobot = jumlah pengesah sah dikurangi jumlah hubungan timbal balik
    /// (pengesah juga disahkan kandidat), sehingga pasangan/cincin saling dukung
    /// kehilangan bobot alih-alih menggandakannya. Perhitungan tidak rekurssif,
    /// sehingga endorsement tidak pernah terakumulasi berantai.
    #[must_use]
    pub fn anti_sybil_weight(&self, candidate: &AccountId) -> u64 {
        let endorsers = self.endorsers_of(candidate);
        let mutual = endorsers
            .iter()
            .filter(|endorser| self.contains(candidate, endorser))
            .count();
        let granted = u64::try_from(endorsers.len()).unwrap_or(u64::MAX);
        let mutual = u64::try_from(mutual).unwrap_or(u64::MAX);
        granted.saturating_sub(mutual)
    }

    /// Komitmen digest kanonikal buku besar endorsement.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_ENDORSEMENT_LEDGER_V1");
        for ((endorser, candidate), entry) in &self.entries {
            hasher.update(endorser);
            hasher.update(candidate);
            hasher.update(&entry.granted_at_block.to_le_bytes());
        }
        for tainted in &self.tainted {
            hasher.update(tainted);
        }
        *hasher.finalize().as_bytes()
    }
}
