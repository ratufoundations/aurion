use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum GuardError {
    #[error(
        "Jumlah Guard aktif kurang dari batas minimum (terdaftar {current}, butuh minimal {min})"
    )]
    InsufficientGuardCount { current: usize, min: usize },

    #[error("Guard {0:?} tidak terdaftar dalam Guard Council")]
    UnauthorizedGuard(PublicKeyBytes),

    #[error("Keputusan pemutusan jaringan gagal: butuh kesepakatan mutlak ({required}/{required}), baru terkumpul {collected}")]
    UnanimousConsentNotMet { collected: usize, required: usize },

    #[error("Tanda tangan keputusan guard tidak sah")]
    InvalidVerdictSignature,

    #[error("Tanda tangan duplikat dari Guard yang sama")]
    DuplicateGuardSignature(PublicKeyBytes),

    #[error("Validator {0:?} tidak berada dalam daftar blacklist")]
    ValidatorNotBlacklisted(PublicKeyBytes),

    #[error("Validator {0:?} sudah terdaftar dalam blacklist")]
    AlreadyBlacklisted(PublicKeyBytes),

    #[error("Kandidat Guard yang memenuhi syarat tidak mencukupi (tersedia {eligible}, dibutuhkan minimal {required})")]
    InsufficientEligibleCandidates { eligible: usize, required: usize },
}
