use aurion_criptografi::PublicKeyBytes;
use std::collections::BTreeMap;

/// Penyimpanan State Kontrak Terisolasi: Contract Address -> (Key u64 -> Value u64)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractStorage {
    state: BTreeMap<PublicKeyBytes, BTreeMap<u64, u64>>,
}

impl ContractStorage {
    pub fn new() -> Self {
        Self {
            state: BTreeMap::new(),
        }
    }

    pub fn get(&self, contract: &PublicKeyBytes, key: u64) -> u64 {
        self.state
            .get(contract)
            .and_then(|store| store.get(&key))
            .copied()
            .unwrap_or(0)
    }

    pub fn set(&mut self, contract: &PublicKeyBytes, key: u64, value: u64) {
        let store = self.state.entry(*contract).or_default();
        if value == 0 {
            store.remove(&key);
        } else {
            store.insert(key, value);
        }
    }
}
