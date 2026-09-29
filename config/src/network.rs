#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const DEFAULT_CHAIN_ID: u64 = 1001;
pub const DEFAULT_P2P_PORT: u16 = 8080;
pub const DEFAULT_RPC_PORT: u16 = 8545;
pub const DEFAULT_BLOCK_TIME_MS: u64 = 2_000; // 2 detik per blok

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub chain_id: u64,
    pub p2p_bind_addr: String,
    pub rpc_bind_addr: String,
    pub block_time_ms: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            chain_id: DEFAULT_CHAIN_ID,
            p2p_bind_addr: format!("0.0.0.0:{DEFAULT_P2P_PORT}"),
            rpc_bind_addr: format!("127.0.0.1:{DEFAULT_RPC_PORT}"),
            block_time_ms: DEFAULT_BLOCK_TIME_MS,
        }
    }
}
