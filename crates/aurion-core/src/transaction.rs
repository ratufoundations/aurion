use crate::error::ExecutionError;
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub sender: PublicKeyBytes,
    pub recipient: PublicKeyBytes,
    pub amount: u64,
    pub nonce: u64,
    pub signature: SignatureBytes,
}

impl Transaction {
    pub const fn new(
        sender: PublicKeyBytes,
        recipient: PublicKeyBytes,
        amount: u64,
        nonce: u64,
        signature: SignatureBytes,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            nonce,
            signature,
        }
    }

    /// Serialisasi kanonikal payload transaksi sebelum di-hash
    pub fn payload_bytes(
        sender: &PublicKeyBytes,
        recipient: &PublicKeyBytes,
        amount: u64,
        nonce: u64,
    ) -> [u8; 80] {
        let mut bytes = [0u8; 80];
        bytes[0..32].copy_from_slice(sender);
        bytes[32..64].copy_from_slice(recipient);
        bytes[64..72].copy_from_slice(&amount.to_le_bytes());
        bytes[72..80].copy_from_slice(&nonce.to_le_bytes());
        bytes
    }

    /// Hash identitas transaksi dengan domain separation
    pub fn digest(&self) -> Hash256 {
        let payload = Self::payload_bytes(&self.sender, &self.recipient, self.amount, self.nonce);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_TX_CANONICAL_V1");
        hasher.update(&payload);
        *hasher.finalize().as_bytes()
    }

    /// Validasi kriptografi tanda tangan transaksi
    pub fn verify_signature(&self) -> Result<(), ExecutionError> {
        let digest = self.digest();
        SignatureVerifier::verify_single(&self.sender, &digest, &self.signature)
            .map_err(|_| ExecutionError::InvalidSignature)
    }
}
