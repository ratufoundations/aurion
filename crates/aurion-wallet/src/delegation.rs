use crate::{domain::DOMAIN_DEVICE_DELEGATION, error::WalletError};
use aurion_criptografi::{Hash256, PublicKeyBytes, SignatureBytes, SignatureVerifier};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCertificate {
    pub identity: PublicKeyBytes,
    pub device_key: PublicKeyBytes,
    pub created_at: u64,
    pub expires_at: u64,
    pub domain_tag: Hash256,
    pub signature: SignatureBytes,
}

impl DeviceCertificate {
    /// Menghitung komitmen biner sertifikat delegasi dengan pemisahan domain
    pub fn digest(
        identity: &PublicKeyBytes,
        device_key: &PublicKeyBytes,
        created_at: u64,
        expires_at: u64,
        domain_tag: &Hash256,
    ) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DOMAIN_DEVICE_DELEGATION);
        hasher.update(identity);
        hasher.update(device_key);
        hasher.update(&created_at.to_le_bytes());
        hasher.update(&expires_at.to_le_bytes());
        hasher.update(domain_tag);
        *hasher.finalize().as_bytes()
    }

    /// Verifikasi keabsahan sertifikat perangkat dan masa berlakunya
    pub fn verify(&self, current_time: u64, expected_domain: &Hash256) -> Result<(), WalletError> {
        if &self.domain_tag != expected_domain {
            return Err(WalletError::DomainMismatch);
        }
        if current_time > self.expires_at {
            return Err(WalletError::DelegationExpired {
                expired_at: self.expires_at,
                current_time,
            });
        }
        let digest = Self::digest(
            &self.identity,
            &self.device_key,
            self.created_at,
            self.expires_at,
            &self.domain_tag,
        );
        SignatureVerifier::verify_single(&self.identity, &digest, &self.signature)
            .map_err(|_| WalletError::InvalidDelegationSignature)?;
        Ok(())
    }
}
