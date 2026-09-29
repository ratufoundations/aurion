use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

/// Tipe galat terstruktur untuk subsistem Guard (penegakan keamanan).
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum GuardError {
    // ========================================================================
    // [G0] Verifikasi Bukti Kriptografis Equivocation
    // ========================================================================

    #[error("Tanda tangan bukti equivocation tidak valid: {0}")]
    InvalidEvidenceSignature(String),

    #[error("Bukti equivocation memiliki hash blok yang identik (bukan pelanggaran): validator {validator:?} di height {height} round {round}")]
    NonConflictingEvidence {
        validator: PublicKeyBytes,
        height: u64,
        round: u32,
    },

    #[error("Bukti equivocation memiliki height/round yang tidak cocok: validator {validator:?} (height_a={height_a}, round_a={round_a}) vs (height_b={height_b}, round_b={round_b})")]
    MismatchedHeightOrRound {
        validator: PublicKeyBytes,
        height_a: u64,
        round_a: u32,
        height_b: u64,
        round_b: u32,
    },

    #[error("Validator {0:?} tidak terdaftar dalam validator set aktif")]
    UnknownValidator(PublicKeyBytes),

    // ========================================================================
    // [G1] Proteksi Replay & Kedaluwarsa Bukti
    // ========================================================================

    #[error("Bukti dengan digest {digest:?} sudah pernah dieksekusi sebelumnya")]
    EvidenceAlreadyExecuted { digest: [u8; 32] },

    #[error("Bukti kedaluwarsa: tinggi blok bukti {evidence_height} melebihi jendela maksimal (current_height={current_height}, max_age={max_age})")]
    EvidenceExpired {
        evidence_height: u64,
        current_height: u64,
        max_age: u64,
    },

    #[error("Upaya eksekusi ganda bukti dalam batch yang sama")]
    DuplicateExecutionInBatch,

    // ========================================================================
    // [G2] Eksekusi Slashing Bertingkat & Konservasi Solvensi
    // ========================================================================

    #[error("Saldo stake validator {validator:?} tidak mencukupi untuk pemotongan: tersedia {available}, dibutuhkan {required}")]
    InsufficientStakeForSlashing {
        validator: PublicKeyBytes,
        available: u64,
        required: u64,
    },

    #[error("Perhitungan slashing overflow: {details}")]
    SlashingOverflow { details: String },

    #[error("Total alokasi denda melebihi amount slash: total_allocated={total_allocated}, slash_amount={slash_amount}")]
    SlashingAllocationExceeds {
        total_allocated: u64,
        slash_amount: u64,
    },

    // ========================================================================
    // [G3] Isolasi Status: Tombstoning & Auto-Jail
    // ========================================================================

    #[error("Validator {0:?} sudah berada dalam status Tombstoned")]
    AlreadyTombstoned(PublicKeyBytes),

    #[error("Validator {0:?} tidak dapat dipulihkan: status Tombstoned adalah permanen")]
    TombstonedIrreversible(PublicKeyBytes),

    // ========================================================================
    // [G4] Otorisasi & Kuorum Dewan Pengawas
    // ========================================================================

    #[error("Jumlah Guard aktif kurang dari batas minimum (terdaftar {current}, butuh minimal {min})")]
    InsufficientGuardCount { current: usize, min: usize },

    #[error("Guard {0:?} tidak terdaftar dalam Guard Council")]
    UnauthorizedGuard(PublicKeyBytes),

    #[error("Tindakan dewan pengawas memerlukan kuorum {required}, terkumpul {collected}")]
    InsufficientCouncilQuorum { collected: usize, required: usize },

    #[error("Tanda tangan keputusan guard tidak sah")]
    InvalidVerdictSignature,

    #[error("Tanda tangan duplikat dari Guard yang sama: {0:?}")]
    DuplicateGuardSignature(PublicKeyBytes),

    #[error("Validator {0:?} tidak berada dalam daftar blacklist")]
    ValidatorNotBlacklisted(PublicKeyBytes),

    #[error("Validator {0:?} sudah terdaftar dalam blacklist")]
    AlreadyBlacklisted(PublicKeyBytes),

    #[error("Aksi darurat: pengaju {caller:?} tidak memiliki peran GuardCouncil")]
    UnauthorizedCouncilAction { caller: PublicKeyBytes },

    #[error("Tanda tangan anggota council tidak valid: {guard:?}")]
    InvalidCouncilMember { guard: PublicKeyBytes },

    #[error("Tanda tangan multi-sig dewan terikat pada chain_id/{chain_id} dan nonce/{nonce}, tanda tangan tidak cocok")]
    CouncilSignatureChainMismatch { chain_id: u64, nonce: u64 },

    // ========================================================================
    // [G5] Akuntansi Denda Nir-Pecahan Berbasis BPS
    // ========================================================================

    #[error("Rasio BPS tidak valid: {bps} (harus <= 10000)")]
    InvalidBpsRatio { bps: u64 },

    #[error("Pembulatan ke bawah (floor) menyebabkan hilangnya Quanta: {lost_quanta}")]
    FloorDivisionLoss { lost_quanta: u64 },

    // ========================================================================
    // Sistem & Operasional
    // ========================================================================

    #[error("Keputusan pemutusan jaringan gagal: butuh kesepakatan mutlak ({required}/{required}), baru terkumpul {collected}")]
    UnanimousConsentNotMet { collected: usize, required: usize },

    #[error("Kandidat Guard yang memenuhi syarat tidak mencukupi (tersedia {eligible}, dibutuhkan minimal {required})")]
    InsufficientEligibleCandidates { eligible: usize, required: usize },
}
