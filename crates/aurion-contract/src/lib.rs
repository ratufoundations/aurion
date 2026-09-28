#![forbid(unsafe_code)]

pub mod context;
pub mod error;
pub mod gas;
pub mod opcodes;
pub mod storage;
pub mod vm;

pub use context::ExecutionContext;
pub use error::VmError;
pub use gas::GasMeter;
pub use opcodes::Opcode;
pub use storage::ContractStorage;
pub use vm::{AurionVm, MAX_STACK_DEPTH};
