use crate::{
    error::ValidatorError,
    scoring::uptime_bps,
    status::{SuspensionReason, ValidatorStatus},
};
use aurion_account::AccountId;
use aurion_core::types::Quanta;
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::BTreeSet;

/// Rekaman siklus hidup satu validator, dikunci oleh `AccountId` berdaulat.
///
/// Seluruh penghitung bersifat monotonik dan diperbarui dengan operasi
/// bertanda (`checked_*`), sehingga luapan selalu menjadi galat bertipe dan
/// tidak pernah membungkam batas protokol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRecord {
    /// Akun berdaulat pemilik slot validator.
    pub account: AccountId,
    /// Kunci publik konsensus yang dibuktikan melalui `PoP`.
    pub consensus_pubkey: PublicKeyBytes,
    /// Kunci master akun yang berwenang mengesahkan permohonan pemulihan.
    pub master_keys: BTreeSet<PublicKeyBytes>,
    /// Stake terkunci (satuan Quanta `u128`).
    pub stake_quanta: Quanta,
    /// Status siklus hidup saat ini.
    pub status: ValidatorStatus,
    /// Blok saat validator diterima ke masa percobaan.
    pub admitted_at_block: u64,
    /// Blok awal jendela probation berjalan.
    pub probation_start_block: u64,
    /// Jumlah blok tercatat pada jendela probation berjalan.
    pub probation_blocks_observed: u64,
    /// Jumlah blok bertanda tangan pada jendela probation berjalan.
    pub probation_signed_blocks: u64,
    /// Rentetan kegagalan liveness berurutan sepanjang jendela probation.
    pub probation_miss_streak: u64,
    /// Rentetan blok terlewat berurutan pada himpunan aktif.
    pub consecutive_missed_blocks: u64,
    /// Total blok bertanda tangan seumur rekaman.
    pub lifetime_signed_blocks: u64,
    /// Total blok terlewat seumur rekaman.
    pub lifetime_missed_blocks: u64,
    /// Blok saat validator mulai bertugas di `ActiveSet` (0 bila tidak aktif).
    pub active_since_block: u64,
    /// Blok saat penahanan/penangguhan dijatuhkan.
    pub jail_start_block: u64,
    /// Alasan penangguhan (bila berstatus `Suspended`).
    pub suspension_reason: Option<SuspensionReason>,
    /// Nonce otorisasi pemulihan (naik atomik setiap `unjail` sah).
    pub unjail_nonce: u64,
    /// Akumulasi stake yang telah dipotong (`slashing`).
    pub slashed_quanta: Quanta,
    /// Tinggi bukti (evidence) yang memicu karantina ireversibel; `0` bila
    /// rekaman tidak pernah di-tombstone.
    pub tombstone_evidence_height: u64,
}

impl ValidatorRecord {
    /// Bangun rekaman validator baru pada status `Probation`.
    #[must_use]
    pub(crate) fn new(
        account: AccountId,
        consensus_pubkey: PublicKeyBytes,
        master_keys: BTreeSet<PublicKeyBytes>,
        stake_quanta: Quanta,
        admitted_at_block: u64,
    ) -> Self {
        Self {
            account,
            consensus_pubkey,
            master_keys,
            stake_quanta,
            status: ValidatorStatus::Probation,
            admitted_at_block,
            probation_start_block: admitted_at_block,
            probation_blocks_observed: 0,
            probation_signed_blocks: 0,
            probation_miss_streak: 0,
            consecutive_missed_blocks: 0,
            lifetime_signed_blocks: 0,
            lifetime_missed_blocks: 0,
            active_since_block: 0,
            jail_start_block: 0,
            suspension_reason: None,
            unjail_nonce: 0,
            slashed_quanta: 0,
            tombstone_evidence_height: 0,
        }
    }

    /// Skor kinerja seumur rekaman dalam basis poin.
    ///
    /// Rekaman tanpa blok yang dapat dinilai dianggap berkinerja penuh sehingga
    /// validator baru tidak dihukum sebelum ada bukti liveness.
    ///
    /// # Errors
    /// Mengembalikan `ValidatorError::ArithmeticOverflow` bila akumulasi blok
    /// meluap dan `ValidatorError::InvalidBlockAccounting` bila pencatatan blok
    /// tidak konsisten.
    pub fn performance_bps(&self) -> Result<u64, ValidatorError> {
        let total = self
            .lifetime_signed_blocks
            .checked_add(self.lifetime_missed_blocks)
            .ok_or(ValidatorError::ArithmeticOverflow)?;
        uptime_bps(self.lifetime_signed_blocks, total)
    }

    /// Masa kerja (*tenure*) dalam blok; nol bila validator tidak di himpunan aktif.
    #[must_use]
    pub fn tenure_blocks(&self, current_block: u64) -> u64 {
        if self.status != ValidatorStatus::ActiveSet {
            return 0;
        }
        current_block.saturating_sub(self.active_since_block)
    }

    /// `true` bila kunci termasuk kunci master akun yang tercatat.
    #[must_use]
    pub fn is_master_key(&self, candidate: &PublicKeyBytes) -> bool {
        self.master_keys.contains(candidate)
    }

    /// Komitmen digest kanonikal rekaman validator.
    #[must_use]
    pub fn state_digest(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_VALIDATOR_RECORD_V1");
        hasher.update(&self.account);
        hasher.update(&self.consensus_pubkey);
        hasher.update(&self.stake_quanta.to_le_bytes());
        hasher.update(&[self.status.code()]);
        hasher.update(&self.probation_blocks_observed.to_le_bytes());
        hasher.update(&self.consecutive_missed_blocks.to_le_bytes());
        hasher.update(&self.lifetime_signed_blocks.to_le_bytes());
        hasher.update(&self.lifetime_missed_blocks.to_le_bytes());
        hasher.update(&self.active_since_block.to_le_bytes());
        hasher.update(&self.jail_start_block.to_le_bytes());
        hasher.update(&self.unjail_nonce.to_le_bytes());
        hasher.update(&self.slashed_quanta.to_le_bytes());
        hasher.update(&self.tombstone_evidence_height.to_le_bytes());
        hasher.update(&[match self.suspension_reason {
            Some(reason) => reason.code(),
            None => u8::MAX,
        }]);
        *hasher.finalize().as_bytes()
    }
}
