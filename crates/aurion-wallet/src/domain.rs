use aurion_criptografi::Hash256;

/// Tag pemisahan domain kanonikal untuk ekosistem wallet Aurion
pub const DOMAIN_DEVICE_DELEGATION: &[u8] = b"AURION_WALLET_DEVICE_DELEGATION_V1";
pub const DOMAIN_TRANSACTION_SIGNING: &[u8] = b"AURION_TX_CANONICAL_V1";
pub const DOMAIN_IDENTITY_CHALLENGE: &[u8] = b"AURION_WALLET_IDENTITY_AUTH_V1";

/// Namespace aplikasi untuk pengujian domain-mismatch.
pub const DOMAIN_TEST_ATTACKER: &str = "attacker.fake.domain";

pub struct Domain;

impl Domain {
    /// Menghasilkan tag 32 bita unik untuk namespace aplikasi / federasi tertentu
    pub fn custom(namespace: &str) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_WALLET_CUSTOM_DOMAIN_V1");
        hasher.update(namespace.as_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Hash payload transaksi 80 bita dengan domain AURION_TX_CANONICAL_V1.
/// Setara byte-per-byte dengan `Transaction::digest()` di aurion-core
/// (BLAKE3 streaming: update(domain) + update(payload)).
pub fn hash_transaction_payload(payload_80b: &[u8; 80]) -> Hash256 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(DOMAIN_TRANSACTION_SIGNING);
    hasher.update(payload_80b);
    *hasher.finalize().as_bytes()
}
