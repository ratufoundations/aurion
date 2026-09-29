#![forbid(unsafe_code)]

use crate::mode::AppMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AurionSettings {
    pub mode: AppMode,
    pub network: NetworkSettings,
    pub consensus: ConsensusSettings,
    pub storage: StorageSettings,
    pub gateway: GatewaySettings,
    pub explorer: ExplorerSettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSettings {
    pub chain_id: u64,
    pub p2p_bind_addr: String,
    pub max_peers: u32,
    pub handshake_timeout_ms: u64,
    pub seed_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsensusSettings {
    pub block_time_ms: u64,
    pub round_timeout_ms: u64,
    pub max_tx_per_block: u32,
    pub quorum_threshold_percent: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    pub db_path: PathBuf,
    pub max_open_files: u32,
    pub flush_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewaySettings {
    pub zenoh_router_endpoint: Option<String>,
    pub query_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExplorerSettings {
    pub http_bind_addr: String,
    pub cors_enabled: bool,
    pub cache_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingSettings {
    pub log_dir: PathBuf,
    pub log_to_stdout: bool,
    pub log_to_file: bool,
    pub max_file_size_mb: u64,
}

impl Default for AurionSettings {
    fn default() -> Self {
        Self {
            mode: AppMode::Developer,
            network: NetworkSettings::default(),
            consensus: ConsensusSettings::default(),
            storage: StorageSettings::default(),
            gateway: GatewaySettings::default(),
            explorer: ExplorerSettings::default(),
            logging: LoggingSettings::default(),
        }
    }
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            chain_id: 1001,
            p2p_bind_addr: "127.0.0.1:8080".to_owned(),
            max_peers: 50,
            handshake_timeout_ms: 5_000,
            seed_nodes: Vec::new(),
        }
    }
}

impl Default for ConsensusSettings {
    fn default() -> Self {
        Self {
            block_time_ms: 2_000,
            round_timeout_ms: 4_000,
            max_tx_per_block: 5_000,
            quorum_threshold_percent: 67,
        }
    }
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("target/aurion_data"),
            max_open_files: 512,
            flush_interval_ms: 1_000,
        }
    }
}

impl Default for GatewaySettings {
    fn default() -> Self {
        Self {
            zenoh_router_endpoint: None,
            query_timeout_ms: 3_000,
        }
    }
}

impl Default for ExplorerSettings {
    fn default() -> Self {
        Self {
            http_bind_addr: "127.0.0.1:8545".to_owned(),
            cors_enabled: true,
            cache_ttl_seconds: 5,
        }
    }
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("logs"),
            log_to_stdout: true,
            log_to_file: true,
            max_file_size_mb: 50,
        }
    }
}
