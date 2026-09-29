use crate::error::ExecutionError;
use crate::fuel::{FuelMeter, FUEL_COST_READ, FUEL_COST_WRITE};
use std::collections::BTreeMap;

pub const WRITE_SET_DOMAIN_TAG: &[u8] = b"AURION_WRITE_SET_DIGEST_V1";

/// Daftar delta mutasi state yang terurut leksikografis biner secara deterministik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSet {
    pub entries: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl WriteSet {
    #[must_use]
    pub fn new(entries: BTreeMap<Vec<u8>, Option<Vec<u8>>>) -> Self {
        Self { entries }
    }

    pub fn apply(&self, target: &mut BTreeMap<Vec<u8>, Vec<u8>>) {
        for (key, val_opt) in &self.entries {
            match val_opt {
                Some(val) => {
                    target.insert(key.clone(), val.clone());
                }
                None => {
                    target.remove(key);
                }
            }
        }
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(WRITE_SET_DOMAIN_TAG);
        hasher.update(&(self.entries.len() as u64).to_be_bytes());

        for (k, v) in &self.entries {
            hasher.update(&(k.len() as u64).to_be_bytes());
            hasher.update(k);
            match v {
                Some(bytes) => {
                    hasher.update(&1u8.to_be_bytes());
                    hasher.update(&(bytes.len() as u64).to_be_bytes());
                    hasher.update(bytes);
                }
                None => {
                    hasher.update(&0u8.to_be_bytes());
                }
            }
        }
        *hasher.finalize().as_bytes()
    }
}

/// Scratchpad Copy-on-Write untuk atomisitas eksekusi satu transaksi.
#[derive(Debug)]
pub struct TransactionalCache {
    base: BTreeMap<Vec<u8>, Vec<u8>>,
    overlay: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    fuel: FuelMeter,
}

impl TransactionalCache {
    #[must_use]
    pub fn new(base: BTreeMap<Vec<u8>, Vec<u8>>, fuel_limit: u64) -> Self {
        Self {
            base,
            overlay: BTreeMap::new(),
            fuel: FuelMeter::new(fuel_limit),
        }
    }

    pub fn get(&mut self, qualified_key: &[u8]) -> Result<Option<Vec<u8>>, ExecutionError> {
        self.fuel.charge(FUEL_COST_READ)?;

        if let Some(staged) = self.overlay.get(qualified_key) {
            return Ok(staged.clone());
        }

        Ok(self.base.get(qualified_key).cloned())
    }

    pub fn set(&mut self, qualified_key: Vec<u8>, value: Vec<u8>) -> Result<(), ExecutionError> {
        self.fuel.charge(FUEL_COST_WRITE)?;
        self.overlay.insert(qualified_key, Some(value));
        Ok(())
    }

    pub fn remove(&mut self, qualified_key: &[u8]) -> Result<(), ExecutionError> {
        self.fuel.charge(FUEL_COST_WRITE)?;
        self.overlay.insert(qualified_key.to_vec(), None);
        Ok(())
    }

    pub fn charge_fuel(&mut self, amount: u64) -> Result<(), ExecutionError> {
        self.fuel.charge(amount)
    }

    #[must_use]
    pub fn fuel_consumed(&self) -> u64 {
        self.fuel.consumed()
    }

    /// Menerapkan perubahan overlay ke basis state jika eksekusi transaksi sukses.
    #[must_use]
    pub fn commit(mut self) -> (WriteSet, BTreeMap<Vec<u8>, Vec<u8>>) {
        let delta = WriteSet::new(self.overlay.clone());
        delta.apply(&mut self.base);
        (delta, self.base)
    }
}
