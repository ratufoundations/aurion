use crate::error::ExecutionError;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub sender: PublicKeyBytes,
    pub recipient: PublicKeyBytes,
    pub amount: u64,
    pub nonce: u64,
    pub fee: u64,
    pub signature: SignatureBytes,
}

impl Transaction {
    #[must_use]
    pub const fn new(
        sender: PublicKeyBytes,
        recipient: PublicKeyBytes,
        amount: u64,
        nonce: u64,
        fee: u64,
        signature: SignatureBytes,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            nonce,
            fee,
            signature,
        }
    }

    /// Canonical signed preimage: nonce || sender || recipient || amount || fee.
    #[must_use]
    pub fn payload_bytes(
        sender: &PublicKeyBytes,
        recipient: &PublicKeyBytes,
        amount: u64,
        nonce: u64,
        fee: u64,
    ) -> [u8; 88] {
        let mut bytes = [0u8; 88];
        bytes[0..8].copy_from_slice(&nonce.to_le_bytes());
        bytes[8..40].copy_from_slice(sender);
        bytes[40..72].copy_from_slice(recipient);
        bytes[72..80].copy_from_slice(&amount.to_le_bytes());
        bytes[80..88].copy_from_slice(&fee.to_le_bytes());
        bytes
    }

    /// Hash canonical transaction fields with domain separation.
    #[must_use]
    pub fn digest(&self) -> Hash256 {
        let payload = Self::payload_bytes(
            &self.sender,
            &self.recipient,
            self.amount,
            self.nonce,
            self.fee,
        );
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_TX_CANONICAL_V1");
        hasher.update(&payload);
        *hasher.finalize().as_bytes()
    }

    /// Validate the sender's cryptographic signature.
    ///
    /// # Errors
    /// Returns an error when the signature does not cover the transaction fields.
    pub fn verify_signature(&self) -> Result<(), ExecutionError> {
        SignatureVerifier::verify_single(&self.sender, &self.digest(), &self.signature)
            .map_err(|_| ExecutionError::InvalidSignature)
    }
}
