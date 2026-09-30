use serde::{Deserialize, Serialize};

/// Manifest metadata untuk satu epoch arsip.
///
/// Hanya memuat tipe integer diskrit dan byte array — tidak ada nilai float (AR5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochManifest {
    pub epoch_index: u64,
    pub start_height: u64,
    pub end_height: u64,
    pub block_count: u32,
    pub final_state_root: [u8; 32],
    pub timestamp: u64,
}

impl EpochManifest {
    /// Serialisasi manifest ke JSON.
    ///
    /// # Errors
    /// Mengembalikan error jika serialisasi gagal.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialisasi dari JSON dengan penolakan float implisit (AR5).
    ///
    /// # Errors
    /// Mengembalikan error jika JSON tidak valid atau mengandung tipe yang tidak diharapkan.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
