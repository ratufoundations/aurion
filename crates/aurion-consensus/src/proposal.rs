use crate::error::ConsensusError;
use aurion_core::Transaction;
use aurion_criptografi::{verifikasi_tanda_tangan, PublicKeyBytes, SignatureBytes};

pub const PROPOSAL_MAGIC: [u8; 4] = [0x50, 0x52, 0x55, 0x41];
pub const HEADER_SIZE: usize = 116;
pub const TX_SIZE: usize = 168;
pub const MIN_PROPOSAL_SIZE: usize = 188;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusBlockHeader {
    pub chain_id: u32,
    pub height: u64,
    pub timestamp: u64,
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub tx_root: [u8; 32],
}

impl ConsensusBlockHeader {
    #[must_use]
    pub fn encode(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.chain_id.to_le_bytes());
        buf[4..12].copy_from_slice(&self.height.to_le_bytes());
        buf[12..20].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[20..52].copy_from_slice(&self.parent_hash);
        buf[52..84].copy_from_slice(&self.state_root);
        buf[84..116].copy_from_slice(&self.tx_root);
        buf
    }

    /// Decode header from 116-byte buffer.
    ///
    /// # Errors
    /// Returns `ConsensusError::InvalidProposalLength` if buffer is not exactly 116 bytes.
    ///
    /// # Panics
    /// Panics if buffer length is not 116 (internal invariant).
    pub fn decode(bytes: &[u8]) -> Result<Self, ConsensusError> {
        if bytes.len() != HEADER_SIZE {
            return Err(ConsensusError::InvalidProposalLength {
                expected: HEADER_SIZE,
                got: bytes.len(),
            });
        }
        let chain_id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let height = u64::from_le_bytes(bytes[4..12].try_into().unwrap_or([0u8; 8]));
        let timestamp = u64::from_le_bytes(bytes[12..20].try_into().unwrap_or([0u8; 8]));
        let mut parent_hash = [0u8; 32];
        parent_hash.copy_from_slice(&bytes[20..52]);
        let mut state_root = [0u8; 32];
        state_root.copy_from_slice(&bytes[52..84]);
        let mut tx_root = [0u8; 32];
        tx_root.copy_from_slice(&bytes[84..116]);
        Ok(Self {
            chain_id,
            height,
            timestamp,
            parent_hash,
            state_root,
            tx_root,
        })
    }

    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        aurion_criptografi::Hasher::digest(&self.encode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockProposal {
    pub header: ConsensusBlockHeader,
    pub transactions: Vec<Transaction>,
    pub proposer_signature: SignatureBytes,
}

impl BlockProposal {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        #[allow(clippy::cast_possible_truncation)]
        let tx_count = self.transactions.len() as u32;
        let mut buf = Vec::with_capacity(MIN_PROPOSAL_SIZE + self.transactions.len() * TX_SIZE);
        buf.extend_from_slice(&PROPOSAL_MAGIC);
        buf.extend_from_slice(&self.header.encode());
        buf.extend_from_slice(&tx_count.to_le_bytes());
        for tx in &self.transactions {
            buf.extend_from_slice(&aurion_ledger::Codec::encode_tx(tx));
        }
        buf.extend_from_slice(&self.proposer_signature);
        buf
    }

    /// Decode proposal from wire buffer.
    ///
    /// # Errors
    /// Returns `ConsensusError` if buffer is too short, magic is invalid, or length mismatch.
    ///
    /// # Panics
    /// Panics if buffer length is insufficient for declared `tx_count`.
    pub fn decode(buf: &[u8]) -> Result<Self, ConsensusError> {
        if buf.len() < MIN_PROPOSAL_SIZE {
            return Err(ConsensusError::InvalidProposalLength {
                expected: MIN_PROPOSAL_SIZE,
                got: buf.len(),
            });
        }
        if buf[0..4] != PROPOSAL_MAGIC {
            return Err(ConsensusError::InvalidProposalMagic);
        }
        let header = ConsensusBlockHeader::decode(&buf[4..120])?;
        let tx_count = u32::from_le_bytes([buf[120], buf[121], buf[122], buf[123]]) as usize;
        let expected_len = MIN_PROPOSAL_SIZE + tx_count * TX_SIZE;
        if buf.len() != expected_len {
            return Err(ConsensusError::InvalidProposalLength {
                expected: expected_len,
                got: buf.len(),
            });
        }
        let mut transactions = Vec::with_capacity(tx_count);
        let mut offset = 124;
        for _ in 0..tx_count {
            let tx_bytes = &buf[offset..offset + TX_SIZE];
            transactions.push(
                aurion_ledger::Codec::decode_tx(tx_bytes)
                    .map_err(|e| ConsensusError::InvalidTransactionInProposal(e.to_string()))?,
            );
            offset += TX_SIZE;
        }
        let mut proposer_signature = [0u8; 64];
        proposer_signature.copy_from_slice(&buf[offset..offset + 64]);
        Ok(Self {
            header,
            transactions,
            proposer_signature,
        })
    }

    /// Verify proposer signature over preimage.
    ///
    /// # Errors
    /// Returns `ConsensusError` if verification fails.
    pub fn verify_signature(
        &self,
        proposer_pubkey: &PublicKeyBytes,
    ) -> Result<bool, ConsensusError> {
        let mut preimage = Vec::with_capacity(156);
        preimage.extend_from_slice(&PROPOSAL_MAGIC);
        preimage.extend_from_slice(&self.header.encode());
        #[allow(clippy::cast_possible_truncation)]
        preimage.extend_from_slice(&(self.transactions.len() as u32).to_le_bytes());
        preimage.extend_from_slice(&self.header.tx_root);
        Ok(verifikasi_tanda_tangan(proposer_pubkey, &preimage, &self.proposer_signature).is_ok())
    }
}
