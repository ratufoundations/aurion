use aurion_criptografi::PublicKeyBytes;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum ConsensusError {
    #[error("Validator {0:?} tidak terdaftar dalam validator set aktif")]
    UnknownValidator(PublicKeyBytes),

    #[error("Tanda tangan vote tidak valid")]
    InvalidVoteSignature,

    #[error("Vote duplikat dari validator yang sama pada ronde ini")]
    DuplicateVote(PublicKeyBytes),

    #[error("Tinggi blok atau ronde tidak cocok")]
    HeightRoundMismatch,

    #[error("Kuorum 2f+1 belum tercapai: terkumpul {collected}, butuh {required}")]
    QuorumNotReached { collected: usize, required: usize },
}
