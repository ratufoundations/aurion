#![forbid(unsafe_code)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]

pub mod action;
pub mod cache;
pub mod capability;
pub mod engine;
pub mod envelope;
pub mod error;
pub mod fuel;
pub mod keeper;
pub mod state_root;
pub mod store_key;

pub use action::{Action, ActionBatch};
pub use cache::{TransactionalCache, WriteSet};
pub use capability::{Capability, CapabilityGrant, CapabilityHandle, CapabilityRegistry, ModuleId};
pub use engine::{ExecutionContext, ExecutionEngine, ExecutionOutcome, ExecutionPolicy};
pub use envelope::TransactionEnvelope;
pub use error::ExecutionError;
pub use fuel::FuelMeter;
pub use keeper::{
    AccountKeeper, ArbitraryModule, GovernanceKeeper, Keeper, StakingKeeper, ValidatorKeeper,
};
pub use state_root::compute_state_root;
pub use store_key::{NamespaceStore, StoreKey};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_smoke_initialization() {
        let policy = ExecutionPolicy::default();
        let engine = ExecutionEngine::new(policy, [1u8; 32]);
        assert!(engine.is_ok());
    }
}
