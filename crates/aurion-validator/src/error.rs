use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

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
