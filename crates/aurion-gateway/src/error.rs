use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Kesalahan Zenoh: {0}")]
    Zenoh(#[from] zenoh::Error),

    #[error("Payload transaksi korup atau tidak valid: {0}")]
    InvalidTransactionPayload(String),

    #[error("Alamat key expression tidak dikenal: {0}")]
    UnknownKeyExpression(String),

    #[error("Kesalahan pembacaan ledger: {0}")]
    Ledger(#[from] aurion_ledger::LedgerError),

    #[error("Penolakan dari antrean mempool: {0}")]
    MempoolRejected(#[from] aurion_mempool::MempoolError),
}
