#![allow(clippy::doc_markdown, clippy::missing_panics_doc, clippy::missing_errors_doc, clippy::clone_on_copy, clippy::cast_possible_truncation)]
//! # Aurion Projection Engine
//!
//! Modul `aurion-projection` adalah mesin pengelola model baca (*Read Model*)
//! dalam arsitektur CQRS protokol Aurion. Modul ini mengonsumsi aliran delta
//! mutasi state (*WriteSet*) dan blok terfinalisasi untuk membangun indeks
//! kueri cepat yang dioptimalkan untuk pembacaan eksternal.
//!
//! ## Arsitektur
//!
//! ```text
//! [Blok Terfinalisasi (L0)]          [WriteSet State Delta (E5)]
//!         |                                   |
//!         +-------------------+-------------------+
//!                         |
//!                         v
//! [P0] Ingestion Pipeline & Idempotent Cursor Guard
//!         |
//!         +-------------------+-------------------+
//!         v                                   v
//! [P1] Address Index          [P2] Read View Snapshot
//!         |                                   |
//!         +-------------------+-------------------+
//!                         |
//! [P3] Pruning & Retention     [P5] Zero-Float Metrics
//! ```
//!
//! ## Fitur Utama
//!
//! - **P0**: Konsumsi delta & idempotensi proyeksi
//! - **P1**: Indeksasi riwayat transaksi berbasis alamat
//! - **P2**: Konsistensi snapshot model baca & deteksi lag
//! - **P3**: Batas retensi & pembersihan aman histori usang
//! - **P4**: Pemulihan kerusakan & rekonstruksi dingin deterministik
//! - **P5**: Metrik kinerja & rasio kuota nir-pecahan

#![forbid(unsafe_code)]

pub mod address_index;
pub mod cursor;
pub mod error;
pub mod metrics;
pub mod pruning;
pub mod recovery;
pub mod snapshot;

// Re-export types for convenience
pub use address_index::{AddressIndex, TransactionEntry, MAX_PAGE_LIMIT};
pub use cursor::{ProjectionCursor, INITIAL_CURSOR};
pub use error::ProjectionError;
pub use metrics::{ProjectionMetrics, BPS_SCALE};
pub use pruning::{PruningConfig, PruningManager, DEFAULT_RETENTION_WINDOW};
pub use recovery::ProjectionRecovery;
pub use snapshot::{ReadSnapshot, SnapshotManager};

/// Konstanta ukuran halaman default.
pub const DEFAULT_PAGE_LIMIT: u64 = 100;

/// Modul utama yang menyatukan seluruh komponen proyeksi.
#[derive(Debug)]
pub struct ProjectionEngine;

impl Default for ProjectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectionEngine {
    /// Inisialisasi mesin proyeksi.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}
