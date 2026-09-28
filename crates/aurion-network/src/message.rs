use crate::error::NetworkError;
use aurion_consensus::{Vote, VoteType};
use aurion_core::{Block, Transaction};
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes};

pub const AURION_NET_MAGIC: u32 = 0x4155524E;
pub const MAX_FRAME_SIZE: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub node_id: PublicKeyBytes,
    pub chain_id: u64,
    pub listen_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkMessage {
    Handshake(Handshake),
    Transaction(Transaction),
    Block(Block),
    Vote(Vote),
    Ping(u64),
    Pong(u64),
}

impl NetworkMessage {
    pub fn message_type_id(&self) -> u8 {
        match self {
            Self::Handshake(_) => 0x01,
            Self::Transaction(_) => 0x02,
            Self::Block(_) => 0x03,
            Self::Vote(_) => 0x04,
            Self::Ping(_) => 0x05,
            Self::Pong(_) => 0x06,
        }
    }

    /// Serialisasi bita Vote BFT: 32 + 32 + 8 + 4 + 1 + 64 = 141 bita
    pub fn encode_vote(vote: &Vote) -> [u8; 141] {
        let mut buf = [0u8; 141];
        buf[0..32].copy_from_slice(&vote.validator);
        buf[32..64].copy_from_slice(&vote.block_hash);
        buf[64..72].copy_from_slice(&vote.height.to_le_bytes());
        buf[72..76].copy_from_slice(&vote.round.to_le_bytes());
        buf[76] = match vote.vote_type {
            VoteType::Prevote => 0x01,
            VoteType::Precommit => 0x02,
        };
        buf[77..141].copy_from_slice(&vote.signature);
        buf
    }

    /// Deserialisasi bita Vote BFT
    pub fn decode_vote(slice: &[u8]) -> Result<Vote, NetworkError> {
        if slice.len() != 141 {
            return Err(NetworkError::MalformedPayload);
        }
        let validator: PublicKeyBytes = slice[0..32].try_into().unwrap();
        let block_hash: Hash256 = slice[32..64].try_into().unwrap();
        let height = u64::from_le_bytes(slice[64..72].try_into().unwrap());
        let round = u32::from_le_bytes(slice[72..76].try_into().unwrap());
        let vote_type = match slice[76] {
            0x01 => VoteType::Prevote,
            0x02 => VoteType::Precommit,
            _ => return Err(NetworkError::MalformedPayload),
        };
        let signature: SignatureBytes = slice[77..141].try_into().unwrap();
        Ok(Vote::new(
            validator, block_hash, height, round, vote_type, signature,
        ))
    }
}
