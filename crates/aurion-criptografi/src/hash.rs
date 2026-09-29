use crate::Hash256;

#[derive(Debug)]
pub struct Hasher;

impl Hasher {
    /// Hashing satu payload secara instan memanfaatkan SIMD hardware.
    #[inline(always)]
    pub fn digest(data: &[u8]) -> Hash256 {
        *blake3::hash(data).as_bytes()
    }

    /// Hashing paralel untuk data besar (chunked).
    pub fn digest_parallel(data: &[u8]) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update_rayon(data);
        *hasher.finalize().as_bytes()
    }

    /// Menggabungkan dua node hash (Merkle tree binary node).
    #[inline(always)]
    pub fn combine(left: &Hash256, right: &Hash256) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(left);
        hasher.update(right);
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_hash() {
        let h1 = Hasher::digest(b"aurion-state-transition");
        let h2 = Hasher::digest(b"aurion-state-transition");
        assert_eq!(h1, h2);
    }
}
