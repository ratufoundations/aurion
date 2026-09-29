use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ApiResponse<T> {
    pub(crate) success: bool,
    pub(crate) data: Option<T>,
    pub(crate) error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub(crate) fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub(crate) fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ChainStatusDto {
    pub(crate) chain_id: u64,
    pub(crate) latest_height: u64,
    pub(crate) latest_block_hash: String,
    pub(crate) latest_state_root: String,
    pub(crate) server_time: u64,
}

#[derive(Serialize)]
pub(crate) struct TransactionDto {
    pub(crate) sender: String,
    pub(crate) recipient: String,
    pub(crate) amount: u64,
    pub(crate) nonce: u64,
    pub(crate) signature: String,
    pub(crate) tx_hash: String,
}

#[derive(Serialize)]
pub(crate) struct BlockDto {
    pub(crate) height: u64,
    pub(crate) block_hash: String,
    pub(crate) prev_hash: String,
    pub(crate) state_root: String,
    pub(crate) tx_count: u32,
    pub(crate) transactions: Vec<TransactionDto>,
}

#[derive(Serialize)]
pub(crate) struct AccountDto {
    pub(crate) address: String,
    pub(crate) balance: u64,
    pub(crate) nonce: u64,
}
