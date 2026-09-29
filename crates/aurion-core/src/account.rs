use aurion_criptografi::{Hash256, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Account {
    pub balance: u64,
    pub nonce: u64,
}

impl Account {
    #[must_use]
    pub const fn new(balance: u64, nonce: u64) -> Self {
        Self { balance, nonce }
    }

    /// Menghitung commitment hash dari status akun
    #[must_use]
    pub fn hash(&self, pubkey: &[u8; 32]) -> Hash256 {
        let mut buf = [0u8; 48]; // 32 bita pubkey + 8 bita balance + 8 bita nonce
        buf[0..32].copy_from_slice(pubkey);
        buf[32..40].copy_from_slice(&self.balance.to_le_bytes());
        buf[40..48].copy_from_slice(&self.nonce.to_le_bytes());
        Hasher::digest(&buf)
    }
}
