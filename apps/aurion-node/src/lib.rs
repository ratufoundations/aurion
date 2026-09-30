#![forbid(unsafe_code)]
#![allow(
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::cast_possible_truncation
)]
//! # Aurion Node — Orkestrasi Daemon
//!
//! Pustaka integrasi yang merakit seluruh subsistem Aurion menjadi satu simpul
//! otonom. Binary `aurion-node` hanyalah pembungkus tipis atas pustaka ini.
//!
//! ## Prinsip Orchestrasi
//!
//! 1. ** Kepemilikan eksklusif state.** Actor `ChainNode` adalah satu-satunya
//!    pemilik `State`, `LedgerStore`, `Mempool`, `ExecutionEngine`,
//!    `GuardCouncil`, dan read model. Tidak ada `Arc<Mutex<T>>` yang membelah
//!    state rantai antar-utas, sehingga tidak ada `lock contention` pada
//!    *hot path*.
//! 2. **Komunikasi berbasis kanal berbatas.** Seluruh transfer antar-subsistem
//!    melewati `tokio::sync::mpsc` berkapasitas tetap. Pengirim memakai
//!    `try_send`, sehingga beban puncak memicu *backpressure* yang terukur
//!    alih-alih penumpukan memori.
//! 3. **Penutupan terkoordinasi.** Satu `CancellationToken` mengoordinasikan
//!    seluruh actor; saat dipicu, actor menyelesaikan perintah yang sedang
//!    berjalan lalu menutup ledger sehingga state ter-*flush*.
//!
//! ```text
//!   NodeHandle (try_send / oneshot)
//!            │
//!            ▼ bounded mpsc
//!   ┌─────────────────────────┐
//!   │  ChainNode (sole owner) │──► ChainEvent (bounded mpsc)
//!   └─────────────────────────┘
//!            ▲ bounded mpsc (gossip)
//!   SwarmActor (AurionWireCodec)
//! ```

pub mod consensus;
pub mod engine;
pub mod node;
pub mod p2p;

pub use node::{
    ChainCommand, ChainEvent, ChainNode, IngressError, NodeConfig, NodeHandle, Quanta,
    DEFAULT_BLOCK_TIME_MS, DEFAULT_MAX_TX_PER_BLOCK, EVENT_CHANNEL_CAPACITY,
    GOSSIP_CHANNEL_CAPACITY, INGRESS_CHANNEL_CAPACITY, PREFIX_BALANCE,
};
pub use p2p::{P2PConfig, P2PHandle, P2PRuntime, P2pCommand};
