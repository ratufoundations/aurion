use aurion_account::{AccountId, Role};
use aurion_core::types::Quanta;
use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

use crate::status::ValidatorStatus;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum AdmissionError {
    #[error("Node {0:?} sudah terdaftar dalam sistem")]
    AlreadyRegistered(PublicKeyBytes),

    #[error("Node {0:?} tidak ditemukan dalam basis registrasi")]
    NodeNotFound(PublicKeyBytes),

    #[error(
        "Masa uji coba belum selesai: telah berjalan {elapsed} blok, dibutuhkan {required} blok"
    )]
    ProbationNotFinished { elapsed: u64, required: u64 },

    #[error("Tingkat keaktifan (uptime) tidak memenuhi syarat: tercapai {actual}%, dibutuhkan minimal {required}%")]
    InsufficientUptime { actual: u64, required: u64 },

    #[error("Node belum memenuhi syarat untuk masuk ke status Candidate")]
    NotEligibleForCandidacy,

    #[error("Penyetuju {0:?} bukan merupakan validator aktif yang sah")]
    UnauthorizedEndorser(PublicKeyBytes),

    #[error("Dukungan persetujuan duplikat dari validator {0:?}")]
    DuplicateEndorsement(PublicKeyBytes),

    #[error(
        "Jumlah persetujuan tidak mencukupi: terkumpul {collected}, dibutuhkan minimal {required}"
    )]
    InsufficientEndorsements { collected: usize, required: usize },

    #[error("Tanda tangan endorsement atau heartbeat tidak sah")]
    InvalidSignature,

    #[error("Node yang sedang diuji tidak boleh menyetujui dirinya sendiri")]
    SelfEndorsementForbidden,
}

/// Galat siklus hidup validator (V0–V5).
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ValidatorError {
    #[error("Akun {0:?} sudah terdaftar sebagai validator")]
    AlreadyRegistered(AccountId),

    #[error("Akun {0:?} tidak terdaftar sebagai validator")]
    NotRegistered(AccountId),

    #[error("Kunci konsensus {0:?} sudah dipakai validator lain")]
    DuplicateConsensusKey(PublicKeyBytes),

    #[error("Stake tidak mencukupi: tersedia {provided}, dibutuhkan {required}")]
    InsufficientStake { provided: Quanta, required: Quanta },

    #[error("Bukti kepemilikan kunci konsensus (PoP) tidak sah atau terikat akun lain")]
    InvalidProofOfPossession,

    #[error("Peran akun tidak berwenang mendaftar validator: dibutuhkan {expected:?}, dimiliki {actual:?}")]
    UnauthorizedRole { expected: Role, actual: Role },

    #[error("Kebijakan validator tidak valid: {reason}")]
    InvalidValidatorPolicy { reason: &'static str },

    #[error("Masa percobaan belum tuntas: tercatat {observed} blok, dibutuhkan {required} blok")]
    ProbationNotFinished { observed: u64, required: u64 },

    #[error(
        "Masa percobaan gagal: {misses} pelanggaran liveness berurutan pada {observed_blocks} blok tercatat"
    )]
    ProbationFailed { misses: u64, observed_blocks: u64 },

    #[error("Transisi status tidak sah: {from:?} tidak boleh berpindah ke {to:?}")]
    InvalidStatusTransition {
        from: ValidatorStatus,
        to: ValidatorStatus,
    },

    #[error("Validator berstatus {status:?} tidak ikut serta dalam konsensus aktif")]
    NotActiveInConsensus { status: ValidatorStatus },

    #[error("Ketinggian {height} bukan batas epoch: rotasi himpunan validator ditolak")]
    EpochNotBoundary { height: u64 },

    #[error("Kapasitas himpunan validator aktif tidak valid: {requested}")]
    InvalidActiveSetCapacity { requested: usize },

    #[error("Masa denda pemulihan belum selesai: tersisa {remaining} blok")]
    UnjailCooldownActive { remaining: u64 },

    #[error(
        "Otorisasi pemulihan tidak sah: binding akun, nonce, atau tanda tangan master tidak cocok"
    )]
    InvalidUnjailAuthorization,

    #[error("Kuota endorsement terlampaui: {active} aktif, maksimum {max}")]
    EndorsementQuotaExceeded { active: usize, max: usize },

    #[error("Masa kerja pengesah belum memadai: {tenure} blok, dibutuhkan {required} blok")]
    EndorserTenureTooShort { tenure: u64, required: u64 },

    #[error(
        "Pengesah {0:?} telah dicabut kewenangannya (tainted) dan tidak boleh mengesahkan lagi"
    )]
    EndorserTainted(AccountId),

    #[error("Validator tidak boleh mengesahkan dirinya sendiri")]
    SelfEndorsementForbidden,

    #[error("Endorsement duplikat dari {endorser:?} untuk kandidat {candidate:?}")]
    DuplicateEndorsement {
        endorser: AccountId,
        candidate: AccountId,
    },

    #[error("Endorsement dari {endorser:?} untuk kandidat {candidate:?} tidak ditemukan")]
    EndorsementNotFound {
        endorser: AccountId,
        candidate: AccountId,
    },

    #[error("Kandidat {status:?} tidak layak menerima endorsement")]
    EndorsementTargetNotEligible { status: ValidatorStatus },

    #[error("Tarif pemotongan tidak valid: {rate_bps} BPS melebihi 10.000 BPS")]
    InvalidSlashRate { rate_bps: u64 },

    #[error("Pencatatan blok tidak konsisten: {signed} blok bertanda tangan dari {eligible} blok memenuhi syarat")]
    InvalidBlockAccounting { signed: u64, eligible: u64 },

    #[error("Aritmetika Quanta meluap (overflow/underflow)")]
    ArithmeticOverflow,
}
