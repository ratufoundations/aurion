use crate::{error::GenesisError, roles::FederationTopology};
use aurion_core::types::Quanta;
use aurion_criptografi::PublicKeyBytes;
use serde::{Deserialize, Serialize};

/// Rasio presisi kanonikal: 1 AUR = 10^10 Quanta (definisi pusat dari `aurion-core`).
pub use aurion_core::types::QUANTA_PER_AUR;
pub const TOTAL_GENESIS_SUPPLY_AUR: u64 = 66_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisSpec {
    pub chain_id: u64,
    pub network_name: String,
    pub timestamp: u64,
    pub treasury_account: PublicKeyBytes,
    pub initial_supply_aur: u64,
    pub topology: FederationTopology,
}

impl GenesisSpec {
    /// Membuat spesifikasi genesis dengan validator dan guard awal.
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika daftar validator atau guard kosong.
    pub fn new(
        chain_id: u64,
        network_name: impl Into<String>,
        timestamp: u64,
        treasury_account: PublicKeyBytes,
        validators: Vec<PublicKeyBytes>,
        guards: Vec<PublicKeyBytes>,
    ) -> Result<Self, GenesisError> {
        if validators.is_empty() {
            return Err(GenesisError::EmptyValidatorSet);
        }
        if guards.is_empty() {
            return Err(GenesisError::EmptyGuardSet);
        }
        Ok(Self {
            chain_id,
            network_name: network_name.into(),
            timestamp,
            treasury_account,
            initial_supply_aur: TOTAL_GENESIS_SUPPLY_AUR,
            topology: FederationTopology::new(validators, guards),
        })
    }

    /// Menghitung pasokan awal dalam satuan dasar terkecil (Quanta).
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika konversi suplai ke Quanta meluap.
    pub fn total_supply_quanta(&self) -> Result<Quanta, GenesisError> {
        u128::from(self.initial_supply_aur)
            .checked_mul(QUANTA_PER_AUR)
            .ok_or(GenesisError::ArithmeticOverflow)
    }

    /// Menyerialisasi spesifikasi ke JSON berformat rapi.
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika serialisasi JSON gagal.
    pub fn to_json_pretty(&self) -> Result<String, GenesisError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Membaca spesifikasi dari JSON.
    ///
    /// # Errors
    ///
    /// Mengembalikan error jika JSON tidak valid atau tidak cocok dengan skema.
    pub fn from_json_str(json: &str) -> Result<Self, GenesisError> {
        Ok(serde_json::from_str(json)?)
    }
}
