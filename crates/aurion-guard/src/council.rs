use crate::{
    error::GuardError,
    verdict::{BlacklistVerdict, PardonVerdict},
};
use aurion_criptografi::{PublicKeyBytes, SignatureVerifier};
use std::collections::BTreeSet;

pub const MINIMUM_GUARD_QUORUM: usize = 5;

#[derive(Debug)]
pub struct GuardCouncil {
    guards: BTreeSet<PublicKeyBytes>,
    blacklisted_validators: BTreeSet<PublicKeyBytes>,
    tombstoned_validators: BTreeSet<PublicKeyBytes>,
}

impl GuardCouncil {
    /// Membuat dewan dari daftar kunci guard.
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika jumlah guard unik kurang dari kuorum minimum.
    pub fn new(guards: Vec<PublicKeyBytes>) -> Result<Self, GuardError> {
        let guard_set: BTreeSet<PublicKeyBytes> = guards.into_iter().collect();
        if guard_set.len() < MINIMUM_GUARD_QUORUM {
            return Err(GuardError::InsufficientGuardCount {
                current: guard_set.len(),
                min: MINIMUM_GUARD_QUORUM,
            });
        }

        Ok(Self {
            guards: guard_set,
            blacklisted_validators: BTreeSet::new(),
            tombstoned_validators: BTreeSet::new(),
        })
    }

    #[must_use]
    pub fn total_guards(&self) -> usize {
        self.guards.len()
    }

    #[must_use]
    pub fn is_guard(&self, key: &PublicKeyBytes) -> bool {
        self.guards.contains(key)
    }

    #[must_use]
    pub fn is_blacklisted(&self, validator: &PublicKeyBytes) -> bool {
        self.blacklisted_validators.contains(validator)
    }

    /// `true` bila validator sudah di-tombstone (daftar cekal permanen).
    #[must_use]
    pub fn is_tombstoned(&self, validator: &PublicKeyBytes) -> bool {
        self.tombstoned_validators.contains(validator)
    }

    /// Catat kunci konsensus ke daftar cekal permanen (tombstone).
    ///
    /// Dipanggil setelah vonis aklamasi untuk pelanggaran yang mewajibkan
    /// tombstone (double-signing). Registri ini ireversibel: kunci yang sudah
    /// tombstone tidak dapat di-blacklist ulang maupun dipulihkan.
    ///
    /// # Errors
    /// Mengembalikan `GuardError::AlreadyTombstoned` bila kunci sudah tercatat.
    pub fn record_tombstone(&mut self, validator: &PublicKeyBytes) -> Result<(), GuardError> {
        if self.tombstoned_validators.contains(validator) {
            return Err(GuardError::AlreadyTombstoned(*validator));
        }
        self.tombstoned_validators.insert(*validator);
        tracing::error!(node = ?validator, penalty = "tombstone", "Cekal permanen dijatuhkan");
        Ok(())
    }

    /// EKSEKUSI PEMUTUSAN JARINGAN (BLACKLIST):
    /// Wajib diverifikasi tanda tangan seluruh anggota Guard (100% konsensus)
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika validator sudah masuk daftar hitam, kuorum tidak
    /// bulat, ada penanda tangan yang bukan guard, atau tanda tangannya tidak valid.
    pub fn execute_blacklist(&mut self, verdict: &BlacklistVerdict) -> Result<(), GuardError> {
        let target = verdict.evidence.target_validator;
        if self.tombstoned_validators.contains(&target) {
            return Err(GuardError::AlreadyTombstoned(target));
        }
        if self.blacklisted_validators.contains(&target) {
            return Err(GuardError::AlreadyBlacklisted(target));
        }

        // 1. Periksa batas aklamasi mutlak (100% Guards wajib sepakat)
        let total = self.guards.len();
        if verdict.signatures.len() != total {
            return Err(GuardError::UnanimousConsentNotMet {
                collected: verdict.signatures.len(),
                required: total,
            });
        }

        let digest = verdict.digest();

        // 2. Verifikasi tanda tangan kriptografis dari setiap Guard
        for (guard_key, sig) in &verdict.signatures {
            if !self.guards.contains(guard_key) {
                return Err(GuardError::UnauthorizedGuard(*guard_key));
            }

            SignatureVerifier::verify_single(guard_key, &digest, sig)
                .map_err(|_| GuardError::InvalidVerdictSignature)?;
        }

        // 3. Putus jaringan: Masukkan ke daftar isolasi mutlak
        tracing::warn!(node = ?target, block_height = verdict.evidence.block_height, "Indikasi anomali: Razia Guard dipicu");
        self.blacklisted_validators.insert(target);
        tracing::error!(node = ?target, penalty = "blacklist", votes = total, "Putusan vonis aklamasi 5/5 dijatuhkan");
        Ok(())
    }

    /// EKSEKUSI PENGAMPUNAN (PARDON / UN-BLACKLIST):
    /// Menghapus status isolasi jika seluruh Guard sepakat atas petisi pembuktian sportif
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika validator belum masuk daftar hitam, kuorum tidak
    /// bulat, ada penanda tangan yang bukan guard, atau tanda tangannya tidak valid.
    pub fn execute_pardon(&mut self, verdict: &PardonVerdict) -> Result<(), GuardError> {
        let target = verdict.petition.target_validator;
        if self.tombstoned_validators.contains(&target) {
            return Err(GuardError::TombstonedIrreversible(target));
        }
        if !self.blacklisted_validators.contains(&target) {
            return Err(GuardError::ValidatorNotBlacklisted(target));
        }

        // 1. Wajib aklamasi mutlak
        let total = self.guards.len();
        if verdict.signatures.len() != total {
            return Err(GuardError::UnanimousConsentNotMet {
                collected: verdict.signatures.len(),
                required: total,
            });
        }

        let digest = verdict.digest();

        // 2. Verifikasi tanda tangan semua Guard
        for (guard_key, sig) in &verdict.signatures {
            if !self.guards.contains(guard_key) {
                return Err(GuardError::UnauthorizedGuard(*guard_key));
            }

            SignatureVerifier::verify_single(guard_key, &digest, sig)
                .map_err(|_| GuardError::InvalidVerdictSignature)?;
        }

        // 3. Pulihkan hak validator
        self.blacklisted_validators.remove(&target);
        tracing::info!(node = ?target, votes = total, "Putusan pardon Guard dijalankan");
        Ok(())
    }
}
