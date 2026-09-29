#![forbid(unsafe_code)]

pub mod engine;
pub mod error;
pub mod metrics;
pub mod rate_limiter;
pub mod routes;
pub mod sanitizer;

pub use error::GatewayError;
pub use engine::AurionGateway;
pub use metrics::GatewayMetrics;
pub use rate_limiter::RateLimiter;
pub use routes::GatewayRoutes;
pub use sanitizer::{FrameSanitizer, HexSanitizer, JsonSanitizer};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_routes_canonical_formatting() {
        assert_eq!(GatewayRoutes::status(1001), "aurion/1001/status");
        assert_eq!(
            GatewayRoutes::account_wildcard(1001),
            "aurion/1001/account/*"
        );
        assert_eq!(
            GatewayRoutes::account_exact(1001, "0011223344"),
            "aurion/1001/account/0011223344"
        );
        assert_eq!(GatewayRoutes::tx_submit(1001), "aurion/1001/tx/submit");
        assert_eq!(
            GatewayRoutes::events_blocks(1001),
            "aurion/1001/events/blocks"
        );
    }
}
