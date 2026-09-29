use std::collections::BTreeMap;

pub const STATE_ROOT_DOMAIN_TAG: &[u8] = b"AURION_EXECUTION_STATE_ROOT_V1";

/// Menghitung komitmen State Root BLAKE3 dari peta penyimpanan secara deterministik.
#[must_use]
pub fn compute_state_root(state: &BTreeMap<Vec<u8>, Vec<u8>>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(STATE_ROOT_DOMAIN_TAG);
    hasher.update(&(state.len() as u64).to_be_bytes());

    for (k, v) in state {
        hasher.update(&(k.len() as u64).to_be_bytes());
        hasher.update(k);
        hasher.update(&(v.len() as u64).to_be_bytes());
        hasher.update(v);
    }

    *hasher.finalize().as_bytes()
}
