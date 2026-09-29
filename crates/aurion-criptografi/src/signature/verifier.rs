use crate::signature::error::CryptoError;
use crate::{PublicKeyBytes, SignatureBytes};
use ed25519_dalek::{Signature as DalekSignature, Verifier, VerifyingKey};
use rayon::prelude::*;

#[derive(Debug)]
pub struct SignatureVerifier;

impl SignatureVerifier {
    /// Verifikasi satu tanda tangan tunggal (constant-time).
    ///
    /// # Errors
    /// Mengembalikan error bila public key tidak valid atau signature tidak cocok.
    pub fn verify_single(
        public_key: &PublicKeyBytes,
        message: &[u8],
        signature: &SignatureBytes,
    ) -> Result<(), CryptoError> {
        let pk = VerifyingKey::from_bytes(public_key).map_err(|_| CryptoError::InvalidPublicKey)?;
        let sig = DalekSignature::from_bytes(signature);

        pk.verify(message, &sig)
            .map_err(|_| CryptoError::VerificationFailed)
    }

    /// Verifikasi batch paralel multi-core menggunakan Rayon.
    ///
    /// # Errors
    /// Mengembalikan `BatchMismatch` bila panjang key, pesan, dan signature berbeda.
    pub fn verify_batch_parallel(
        public_keys: &[PublicKeyBytes],
        messages: &[&[u8]],
        signatures: &[SignatureBytes],
        chunk_size: usize,
    ) -> Result<bool, CryptoError> {
        if public_keys.len() != messages.len() || messages.len() != signatures.len() {
            return Err(CryptoError::BatchMismatch);
        }

        if public_keys.is_empty() {
            return Ok(true);
        }

        // Siapkan triplet tuple per transaksi.
        let items: Vec<(&PublicKeyBytes, &[u8], &SignatureBytes)> = public_keys
            .iter()
            .zip(messages.iter().copied())
            .zip(signatures.iter())
            .map(|((pk, msg), sig)| (pk, msg, sig))
            .collect();

        // Bagi transaksi ke worker pool Rayon.
        let all_valid = items.par_chunks(chunk_size.max(1)).all(|chunk| {
            let mut chunk_msgs = Vec::with_capacity(chunk.len());
            let mut chunk_sigs = Vec::with_capacity(chunk.len());
            let mut chunk_pks = Vec::with_capacity(chunk.len());

            for (pk, msg, sig) in chunk {
                let Ok(key) = VerifyingKey::from_bytes(pk) else {
                    return false;
                };
                chunk_pks.push(key);
                chunk_sigs.push(DalekSignature::from_bytes(sig));
                chunk_msgs.push(*msg);
            }

            ed25519_dalek::verify_batch(&chunk_msgs, &chunk_sigs, &chunk_pks).is_ok()
        });

        Ok(all_valid)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::signature::keypair::Keypair;

    #[test]
    fn test_sign_and_verify_single() {
        let keypair = Keypair::generate();
        let msg = b"aurion-tx-payload";
        let sig = keypair.sign(msg);

        let result = SignatureVerifier::verify_single(&keypair.public_key_bytes(), msg, &sig);
        assert!(result.is_ok());
    }

    #[test]
    fn test_batch_verify_parallel() {
        let mut pks = Vec::new();
        let mut sigs = Vec::new();
        let messages = vec![&b"tx_payload_data"[..]; 100];

        for _ in 0..100 {
            let kp = Keypair::generate();
            pks.push(kp.public_key_bytes());
            sigs.push(kp.sign(b"tx_payload_data"));
        }

        let is_valid = SignatureVerifier::verify_batch_parallel(&pks, &messages, &sigs, 32)
            .expect("test operation should succeed");

        assert!(is_valid);
    }
}
