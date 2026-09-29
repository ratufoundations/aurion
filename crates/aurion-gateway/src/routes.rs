#[derive(Debug)]
pub struct GatewayRoutes;

impl GatewayRoutes {
    /// `aurion/{chain_id}/status` -> Queryable status rantai
    #[must_use]
    pub fn status(chain_id: u64) -> String {
        format!("aurion/{chain_id}/status")
    }

    /// `aurion/{chain_id}/account`/* -> Queryable saldo dan nonce akun
    #[must_use]
    pub fn account_wildcard(chain_id: u64) -> String {
        format!("aurion/{chain_id}/account/*")
    }

    /// `aurion/{chain_id}/account/{pubkey_hex`}
    #[must_use]
    pub fn account_exact(chain_id: u64, pubkey_hex: &str) -> String {
        format!("aurion/{chain_id}/account/{pubkey_hex}")
    }

    /// `aurion/{chain_id}/tx/submit` -> Queryable / Put untuk kirim raw tx
    #[must_use]
    pub fn tx_submit(chain_id: u64) -> String {
        format!("aurion/{chain_id}/tx/submit")
    }

    /// `aurion/{chain_id}/events/blocks` -> Pub/Sub stream blok baru yang di-commit
    #[must_use]
    pub fn events_blocks(chain_id: u64) -> String {
        format!("aurion/{chain_id}/events/blocks")
    }
}
