#![forbid(unsafe_code)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]

//! Mesin pembayaran mikro native dengan State Channel (pengganti arah
//! arsitektur `aurion-contract`). Mewarisi disiplin protokol Aurion:
//! zero-unsafe, zero-float, dan moneter murni `Quanta` (`u128`).

pub mod error;
pub mod keeper;
pub mod types;

pub use error::ChannelError;
pub use keeper::ChannelKeeper;
pub use types::{
    BalanceProof, ChannelId, ChannelState, ChannelStatus, TICKET_PREIMAGE_SIZE, TICKET_SIZE,
};
