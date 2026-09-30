use crate::{error::ValidatorError, record::ValidatorRecord, scoring::effective_stake_quanta};
use aurion_account::{AccountId, MIN_VALIDATOR_STAKE_QUANTA};
use aurion_core::types::Quanta;
use std::collections::BTreeMap;

/// Panjang satu epoch (batas rotasi himpunan validator) dalam blok.
pub const EPOCH_LENGTH_BLOCKS: u64 = 1_200;

/// Kapasitas maksimum himpunan validator aktif.
pub const MAX_ACTIVE_VALIDATORS: usize = 100;

/// Jadwal batas epoch berbasis aritmetika bilangan bulat murni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpochSchedule {
    /// Panjang epoch dalam blok.
    pub length_blocks: u64,
}

impl EpochSchedule {
    /// Bangun jadwal epoch dengan panjang tertentu.
    #[must_use]
    pub const fn new(length_blocks: u64) -> Self {
        Self { length_blocks }
    }

    /// Nomor epoch yang memuat ketinggian blok ini.
    #[must_use]
    pub const fn epoch_of(self, height: u64) -> u64 {
        if self.length_blocks == 0 {
            return 0;
        }
        height / self.length_blocks
    }

    /// `true` bila ketinggian blok berada tepat pada batas epoch.
    #[must_use]
    pub const fn is_boundary(self, height: u64) -> bool {
        self.length_blocks != 0 && height.is_multiple_of(self.length_blocks)
    }
}

/// Hasil seleksi deterministik himpunan validator aktif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSetSelection {
    /// Epoch tujuan hasil seleksi.
    pub epoch: u64,
    /// Ketinggian blok batas epoch tempat seleksi dihitung.
    pub boundary_height: u64,
    /// Himpunan terpilih, terurut menurut peringkat (bukan leksikografis).
    pub active_set: Vec<AccountId>,
    /// Peringkat penuh seluruh kandidat yang memenuhi syarat: `(akun, skor)`.
    pub ranking: Vec<(AccountId, Quanta)>,
}

/// Hitung komposisi `ActiveSet` berikutnya secara deterministik.
///
/// Urutan prioritas: skor peringkat `f(stake, performance_bps)` menurun, lalu
/// stake mentah menurun, lalu `account_id` menaik sebagai pemutus seri mutlak.
/// Fungsi ini murni: dua simpul dengan state identik wajib menghasilkan
/// `ranking` dan `active_set` yang identik byte-per-byte.
///
/// # Errors
/// Mengembalikan `ValidatorError::EpochNotBoundary` bila ketinggian bukan batas
/// epoch, `ValidatorError::InvalidActiveSetCapacity` bila kapasitas nol, serta
/// galat aritmetika bertipe dari perhitungan skor.
pub fn select_active_set(
    schedule: EpochSchedule,
    height: u64,
    records: &BTreeMap<AccountId, ValidatorRecord>,
    capacity: usize,
) -> Result<ActiveSetSelection, ValidatorError> {
    if !schedule.is_boundary(height) {
        return Err(ValidatorError::EpochNotBoundary { height });
    }
    if capacity == 0 {
        return Err(ValidatorError::InvalidActiveSetCapacity {
            requested: capacity,
        });
    }

    let mut ranked: Vec<(AccountId, Quanta, Quanta)> = Vec::new();
    for (account, record) in records {
        if !record.status.is_selectable() || record.stake_quanta < MIN_VALIDATOR_STAKE_QUANTA {
            continue;
        }
        let score = effective_stake_quanta(record.stake_quanta, record.performance_bps()?)?;
        ranked.push((*account, score, record.stake_quanta));
    }

    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });

    let ranking: Vec<(AccountId, Quanta)> = ranked
        .iter()
        .map(|(account, score, _)| (*account, *score))
        .collect();
    let active_set: Vec<AccountId> = ranked
        .iter()
        .take(capacity)
        .map(|(account, _, _)| *account)
        .collect();

    Ok(ActiveSetSelection {
        epoch: schedule.epoch_of(height),
        boundary_height: height,
        active_set,
        ranking,
    })
}
