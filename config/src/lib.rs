#![forbid(unsafe_code)]

pub mod logger;
pub mod mode;
pub mod network;
pub mod settings;
pub mod validation;

use std::path::Path;
use thiserror::Error;

pub use logger::{init_central_logging, init_central_logging_with_settings};
pub use mode::AppMode;
pub use network::{DEFAULT_BLOCK_TIME_MS, DEFAULT_CHAIN_ID, DEFAULT_P2P_PORT, DEFAULT_RPC_PORT};
pub use settings::{
    AurionSettings, ConsensusSettings, ExplorerSettings, GatewaySettings, LoggingSettings,
    NetworkSettings, StorageSettings,
};
pub use validation::{validate_settings, ValidationError};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Gagal membaca berkas konfigurasi: {0}")]
    Io(#[from] std::io::Error),
    #[error("Format konfigurasi TOML tidak valid: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Invarian konfigurasi gagal: {0}")]
    Validation(#[from] ValidationError),
}

impl AurionSettings {
    /// Membaca TOML, menggabungkan field yang tidak dicantumkan dengan default, lalu memvalidasi invarian.
    ///
    /// # Errors
    /// Mengembalikan error bila file tidak dapat dibaca, TOML tidak valid, atau invarian gagal.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let settings: Self = toml::from_str(&content)?;
        validation::validate_settings(&settings)?;
        Ok(settings)
    }

    /// Memuat konfigurasi dan memakai nilai default bila file tidak tersedia atau tidak valid.
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::load_from_file(path).unwrap_or_default()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn defaults_satisfy_validation() {
        let settings = AurionSettings::default();
        assert_eq!(settings.network.chain_id, DEFAULT_CHAIN_ID);
        assert_eq!(settings.consensus.block_time_ms, DEFAULT_BLOCK_TIME_MS);
        assert!(validate_settings(&settings).is_ok());
    }

    #[test]
    fn loads_developer_blueprint() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("node.dev.toml");
        let settings = AurionSettings::load_from_file(path).expect("developer config should load");
        assert_eq!(settings.mode, AppMode::Developer);
        assert_eq!(
            settings.storage.db_path,
            std::path::Path::new("target/dev_db")
        );
        assert_eq!(settings.explorer.http_bind_addr, "127.0.0.1:8545");
    }

    #[test]
    fn loads_production_blueprint() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("node.prod.toml");
        let settings = AurionSettings::load_from_file(path).expect("production config should load");
        assert_eq!(settings.mode, AppMode::Production);
        assert_eq!(
            settings.storage.db_path,
            std::path::Path::new("/var/lib/aurion/data")
        );
        assert!(!settings.logging.log_to_stdout);
    }

    #[test]
    fn rejects_invalid_quorum() {
        let mut settings = AurionSettings::default();
        settings.consensus.quorum_threshold_percent = 50;
        assert_eq!(
            validate_settings(&settings),
            Err(ValidationError::InvalidQuorum(50))
        );
    }
}
