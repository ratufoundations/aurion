use crate::{delegation::DeviceCertificate, domain::hash_transaction_payload, error::WalletError};
use aurion_core::Transaction;
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};

#[derive(Debug)]
pub struct AurionWallet {
    pub identity: Keypair,
    pub domain_tag: Hash256,
}

impl AurionWallet {
    /// Inisialisasi wallet baru dengan kunci identitas utama dan domain namespace
    #[must_use]
    pub fn new(identity: Keypair, namespace: &str) -> Self {
        let domain_tag = crate::domain::Domain::custom(namespace);
        Self {
            identity,
            domain_tag,
        }
    }

    #[must_use]
    pub fn public_key(&self) -> PublicKeyBytes {
        self.identity.public_key_bytes()
    }

    /// Tautkan perangkat baru (ala scan QR `WhatsApp`) dengan menerbitkan `DeviceCertificate`
    #[must_use]
    pub fn delegate_device(
        &self,
        device_pubkey: PublicKeyBytes,
        current_time: u64,
        validity_duration_seconds: u64,
    ) -> DeviceCertificate {
        let created_at = current_time;
        let expires_at = current_time.saturating_add(validity_duration_seconds);
        let digest = DeviceCertificate::digest(
            &self.identity.public_key_bytes(),
            &device_pubkey,
            created_at,
            expires_at,
            &self.domain_tag,
        );
        let signature = self.identity.sign(&digest);
        DeviceCertificate {
            identity: self.identity.public_key_bytes(),
            device_key: device_pubkey,
            created_at,
            expires_at,
            domain_tag: self.domain_tag,
            signature,
        }
    }

    /// Bangun dan tandatangani transaksi menggunakan Kunci Identitas Utama
    pub fn build_transaction(
        &self,
        recipient: PublicKeyBytes,
        amount: u64,
        nonce: u64,
    ) -> Transaction {
        let sender = self.identity.public_key_bytes();
        let payload = Transaction::payload_bytes(&sender, &recipient, amount, nonce);
        let digest = hash_transaction_payload(&payload);
        let signature = self.identity.sign(&digest);
        tracing::debug!(addr = ?sender, amount, nonce, "Transaksi berhasil ditandatangani oleh dompet");
        Transaction::new(sender, recipient, amount, nonce, signature)
    }
}

/// Sisi Klien Perangkat (misal: Ponsel / Browser) yang hanya memegang `DeviceKey`
#[derive(Debug)]
pub struct LinkedDeviceSession {
    pub device_keypair: Keypair,
    pub certificate: DeviceCertificate,
}

impl LinkedDeviceSession {
    #[must_use]
    pub fn new(device_keypair: Keypair, certificate: DeviceCertificate) -> Self {
        Self {
            device_keypair,
            certificate,
        }
    }

    /// Validasi status izin sesi perangkat terhadap domain dan jam sistem.
    ///
    /// # Errors
    /// Mengembalikan error bila sertifikat perangkat tidak valid.
    pub fn is_valid(
        &self,
        current_time: u64,
        expected_domain: &Hash256,
    ) -> Result<(), WalletError> {
        self.certificate.verify(current_time, expected_domain)
    }
}
