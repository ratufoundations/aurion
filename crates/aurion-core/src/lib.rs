#![forbid(unsafe_code)]

pub mod account;
pub mod block;
pub mod error;
pub mod module;
pub mod state;
pub mod transaction;
pub mod types;

pub use account::Account;
pub use block::{Block, BlockHeader};
pub use error::ExecutionError;
pub use module::{
    AurionModule, DispatchError, ExecutionContext, ModuleDispatcher, StateReader, StateWriter,
};
pub use state::{State, PROTOCOL_FEE_SINK};
pub use transaction::Transaction;
pub use types::{
    calculate_bps, mul_div_quanta, Quanta, BPS_DENOMINATOR, QUANTA_PER_AUR, TREASURY_GENESIS_QUANTA,
};
