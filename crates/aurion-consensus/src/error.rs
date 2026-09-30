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

    #[error("Validator mengirim vote bertentangan pada height, round, dan fase yang sama")]
    EquivocationDetected(PublicKeyBytes),

    #[error("Vote atau timeout vote berasal dari height/round yang sudah lewat")]
    StaleVote,

    #[error("Timeout vote duplikat dari validator yang sama")]
    DuplicateTimeoutVote(PublicKeyBytes),

    #[error("Ronde atau perhitungan kuorum melampaui batas integer")]
    ArithmeticOverflow,

    #[error("Tidak ada validator untuk memilih leader")]
    EmptyValidatorSet,

    #[error("Tinggi blok atau ronde tidak cocok")]
    HeightRoundMismatch,

    #[error("Kuorum 2f+1 belum tercapai: terkumpul {collected}, butuh {required}")]
    QuorumNotReached { collected: usize, required: usize },

    #[error("Panjang proposal tidak valid: diharapkan {expected}, diterima {got}")]
    InvalidProposalLength { expected: usize, got: usize },

    #[error("Magic prefix proposal tidak valid")]
    InvalidProposalMagic,

    #[error("Transaksi dalam proposal tidak valid: {0}")]
    InvalidTransactionInProposal(String),

    #[error("Tanda tangan proposer tidak valid")]
    InvalidProposerSignature,

    #[error("Block hash tidak cocok dengan header")]
    BlockHashMismatch,

    #[error("State root tidak cocok")]
    StateRootMismatch,

    #[error("Tx root tidak cocok")]
    TxRootMismatch,
}
