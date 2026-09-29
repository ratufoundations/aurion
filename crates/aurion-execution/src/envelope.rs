use crate::error::ExecutionError;

pub const ENVELOPE_DOMAIN_TAG: &[u8] = b"AURION_TX_ENVELOPE_V1";

/// Amplop transaksi formal dengan pengikatan chain_id dan tanda tangan Ed25519.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionEnvelope {
    pub version: u16,
    pub chain_id: u64,
    pub nonce: u64,
    pub sender: [u8; 32],
    pub fee: u64,
    pub target_module: [u8; 4],
    pub payload: Vec<u8>,
    pub signature: [u8; 64],
}

impl TransactionEnvelope {
    #[must_use]
    pub fn preimage_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ENVELOPE_DOMAIN_TAG);
        hasher.update(&self.version.to_be_bytes());
        hasher.update(&self.chain_id.to_be_bytes());
        hasher.update(&self.nonce.to_be_bytes());
        hasher.update(&self.sender);
        hasher.update(&self.fee.to_be_bytes());
        hasher.update(&self.target_module);
        hasher.update(&(self.payload.len() as u64).to_be_bytes());
        hasher.update(&self.payload);
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(
        &self,
        expected_chain_id: u64,
    ) -> Result<(), ExecutionError> {
        if self.chain_id != expected_chain_id {
            return Err(ExecutionError::InvalidChainId {
                expected: expected_chain_id,
                got: self.chain_id,
            });
        }

        let digest = self.preimage_hash();
        aurion_criptografi::verifikasi_tanda_tangan(&self.sender, &digest, &self.signature)
            .map_err(|_| ExecutionError::InvalidSignature)
    }
}
