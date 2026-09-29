#![forbid(unsafe_code)]

pub mod logger;
pub mod mode;
pub mod network;

use std::path::Path;
use thiserror::Error;

pub use logger::init_central_logging;
pub use mode::AppMode;
pub use network::{
    NetworkConfig, DEFAULT_BLOCK_TIME_MS, DEFAULT_CHAIN_ID, DEFAULT_P2P_PORT, DEFAULT_RPC_PORT,
};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Gagal membaca berkas konfigurasi: {0}")]
    Io(#[from] std::io::Error),

    #[error("Gagal mengurai format TOML: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AurionConfig {
    pub mode: AppMode,
    pub network: NetworkConfig,
    pub db_path: String,
    pub log_dir: String,
}

impl Default for AurionConfig {
    fn default() -> Self {
        Self {
            mode: AppMode::Developer,
            network: NetworkConfig::default(),
            db_path: "target/aurion_data".to_string(),
            log_dir: "logs".to_string(),
        }
    }
}

impl AurionConfig {
    /// Muat konfigurasi dari file TOML.
    ///
    /// # Errors
    ///
    /// Mengembalikan [`ConfigError::Io`] bila berkas tidak dapat dibaca dan
    /// [`ConfigError::Parse`] bila isi berkas bukan TOML yang valid.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: AurionConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
    }

    #[test]
    fn default_mode_is_developer() {
        assert_eq!(AurionConfig::default().mode, AppMode::Developer);
    }

    #[test]
    fn default_network_matches_constants() {
        let net = NetworkConfig::default();
        assert_eq!(net.chain_id, DEFAULT_CHAIN_ID);
        assert_eq!(net.block_time_ms, DEFAULT_BLOCK_TIME_MS);
        assert_eq!(net.p2p_bind_addr, format!("0.0.0.0:{DEFAULT_P2P_PORT}"));
        assert_eq!(net.rpc_bind_addr, format!("127.0.0.1:{DEFAULT_RPC_PORT}"));
    }

    #[test]
    fn load_dev_toml() {
        match AurionConfig::load_from_file(manifest_path("node.dev.toml")) {
            Ok(cfg) => {
                assert_eq!(cfg.mode, AppMode::Developer);
                assert_eq!(cfg.network.chain_id, DEFAULT_CHAIN_ID);
                assert_eq!(cfg.db_path, "target/dev_db");
            }
            Err(e) => panic!("gagal memuat node.dev.toml: {e}"),
        }
    }

    #[test]
    fn load_prod_toml() {
        match AurionConfig::load_from_file(manifest_path("node.prod.toml")) {
            Ok(cfg) => {
                assert_eq!(cfg.mode, AppMode::Production);
                assert_eq!(cfg.network.chain_id, DEFAULT_CHAIN_ID);
                assert_eq!(cfg.db_path, "/var/lib/aurion/data");
            }
            Err(e) => panic!("gagal memuat node.prod.toml: {e}"),
        }
    }

    #[test]
    fn load_missing_file_returns_io_error() {
        match AurionConfig::load_from_file(manifest_path("tidak-ada.toml")) {
            Ok(_) => panic!("seharusnya gagal untuk berkas yang tidak ada"),
            Err(ConfigError::Io(_)) => {}
            Err(e) => panic!("jenis galat salah: {e}"),
        }
    }
}
