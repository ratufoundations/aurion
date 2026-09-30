use crate::types::Quanta;
use aurion_criptografi::{Hash256, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Account {
    pub balance: Quanta,
    pub nonce: u64,
}

impl Account {
    #[must_use]
    pub const fn new(balance: Quanta, nonce: u64) -> Self {
        Self { balance, nonce }
    }

    /// Menghitung commitment hash dari status akun
    #[must_use]
    pub fn hash(&self, pubkey: &[u8; 32]) -> Hash256 {
        let mut buf = [0u8; 56]; // 32 bita pubkey + 16 bita balance + 8 bita nonce
        buf[0..32].copy_from_slice(pubkey);
        buf[32..48].copy_from_slice(&self.balance.to_le_bytes());
        buf[48..56].copy_from_slice(&self.nonce.to_le_bytes());
        Hasher::digest(&buf)
    }
}
