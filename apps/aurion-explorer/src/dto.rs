use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Serialize)]
pub struct ChainStatusDto {
    pub chain_id: u64,
    pub latest_height: u64,
    pub latest_block_hash: String,
    pub latest_state_root: String,
    pub server_time: u64,
}

#[derive(Serialize)]
pub struct TransactionDto {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
    pub nonce: u64,
    pub signature: String,
    pub tx_hash: String,
}

#[derive(Serialize)]
pub struct BlockDto {
    pub height: u64,
    pub block_hash: String,
    pub prev_hash: String,
    pub state_root: String,
    pub tx_count: u32,
    pub transactions: Vec<TransactionDto>,
}

#[derive(Serialize)]
pub struct AccountDto {
    pub address: String,
    pub balance: u64,
    pub nonce: u64,
}
