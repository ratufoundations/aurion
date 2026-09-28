pub struct GatewayRoutes;

impl GatewayRoutes {
    /// aurion/{chain_id}/status -> Queryable status rantai
    pub fn status(chain_id: u64) -> String {
        format!("aurion/{}/status", chain_id)
    }

    /// aurion/{chain_id}/account/* -> Queryable saldo dan nonce akun
    pub fn account_wildcard(chain_id: u64) -> String {
        format!("aurion/{}/account/*", chain_id)
    }

    /// aurion/{chain_id}/account/{pubkey_hex}
    pub fn account_exact(chain_id: u64, pubkey_hex: &str) -> String {
        format!("aurion/{}/account/{}", chain_id, pubkey_hex)
    }

    /// aurion/{chain_id}/tx/submit -> Queryable / Put untuk kirim raw tx
    pub fn tx_submit(chain_id: u64) -> String {
        format!("aurion/{}/tx/submit", chain_id)
    }

    /// aurion/{chain_id}/events/blocks -> Pub/Sub stream blok baru yang di-commit
    pub fn events_blocks(chain_id: u64) -> String {
        format!("aurion/{}/events/blocks", chain_id)
    }
}
