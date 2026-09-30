#![forbid(unsafe_code)]

pub mod error;
pub mod proposal;
pub mod qc;
pub mod state;
pub mod validator;
pub mod vote;

pub use error::ConsensusError;
pub use proposal::{
    BlockProposal, ConsensusBlockHeader, HEADER_SIZE, MIN_PROPOSAL_SIZE, PROPOSAL_MAGIC,
};
pub use qc::{calculate_quorum, QuorumCertificate};
pub use state::{EquivocationEvidence, RoundState, TimeoutCertificate};
pub use validator::ValidatorSet;
pub use vote::{PrecommitVote, TimeoutVote, Vote, VoteType, PRECOMMIT_VOTE_SIZE};
