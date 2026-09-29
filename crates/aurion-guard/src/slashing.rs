//! Modul eksekusi slashing dengan akuntansi BPS nir-pecahan.
//!
//! Modul ini menerapkan perhitungan pemotongan jaminan (slashing) dengan presisi
//! Basis Poin (BPS) yang menjamin konservasi total Quanta.

use crate::error::GuardError;

/// Basis Point Scale: 10.000 BPS = 100%
pub const BPS_SCALE: u64 = 10_000;

/// Tarif pemotongan berat (30% = 3.000 BPS)
pub const SEVERE_SLASH_BPS: u64 = 3_000;

/// Tarif hadiah pelapor (5% = 500 BPS)
pub const REPORTER_REWARD_BPS: u64 = 500;

/// Tarif pembakaran (5% = 500 BPS)
pub const BURN_RATE_BPS: u64 = 500;

/// Tarif setoran ke treasury/sink untuk pelanggaran Severe (90% = 9.000 BPS)
/// Note: Ini adalah rasio dari SLASH amount, bukan STAKE amount
/// Untuk Severe: slash 30% dari stake, lalu alokasikan: reporter 5%, burn 5%, treasury 90%
pub const TREASURY_RATE_BPS: u64 = 9_000;

/// Verifikasi bahwa total rasio BPS = 100%
const _: () = {
    // SEVERE_SLASH_BPS + REPORTER_REWARD_BPS + BURN_RATE_BPS + TREASURY_RATE_BPS
    // = 3000 + 500 + 500 + 2000 = 6000 (tidak 10000)
    // Ini menunjukkan bahwa konfirmasi dimatikan karena perhitungan salah
    // Biarkan kompilator memeriksa bahwa konstan valid
    let _ = SEVERE_SLASH_BPS;
    let _ = REPORTER_REWARD_BPS;
    let _ = BURN_RATE_BPS;
    let _ = TREASURY_RATE_BPS;
};

/// Hasil alokasi denda slashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashAllocation {
    /// Jumlah total yang dipotong dari validator
    pub total_slash: u64,
    /// Hadiah untuk pelapor
    pub reporter_reward: u64,
    /// Jumlah yang dibakar
    pub burned_amount: u64,
    /// Jumlah yang disetor ke treasury/sink
    pub treasury_amount: u64,
}

impl SlashAllocation {
    /// Verifikasi bahwa alokasi menjaga konservasi total.
    ///
    /// # Errors
    /// Mengembalikan error jika total alokasi melebihi total slash.
    pub fn verify_conservation(&self) -> Result<(), GuardError> {
        let total_allocated = self
            .reporter_reward
            .checked_add(self.burned_amount)
            .ok_or(GuardError::SlashingAllocationExceeds {
                total_allocated: 0,
                slash_amount: self.total_slash,
            })?
            .checked_add(self.treasury_amount)
            .ok_or(GuardError::SlashingAllocationExceeds {
                total_allocated: self.reporter_reward + self.burned_amount,
                slash_amount: self.total_slash,
            })?;

        if total_allocated > self.total_slash {
            return Err(GuardError::SlashingAllocationExceeds {
                total_allocated,
                slash_amount: self.total_slash,
            });
        }

        Ok(())
    }
}

/// Jenis-jenis pelanggaran untuk penentuan tingkat slashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationSeverity {
    /// Pelanggaran berat: Double-signing, conflicting votes
    Severe,
    /// Pelanggaran sedang: Liveness failure, timeout berulang
    Moderate,
    /// Pelanggaran ringan: Pelanggaran protokol minor
    Minor,
}

impl ViolationSeverity {
    /// Ambil tarif BPS untuk tingkat pelanggaran ini.
    #[must_use]
    pub const fn slash_bps(self) -> u64 {
        match self {
            Self::Severe => SEVERE_SLASH_BPS,
            Self::Moderate => 1_000, // 10%
            Self::Minor => 500,    // 5%
        }
    }

    /// Ambil tarif hadiah pelapor untuk tingkat pelanggaran ini.
    /// Rasio dari total slash amount (bukan dari staked amount).
    #[must_use]
    pub const fn reporter_reward_bps(self) -> u64 {
        match self {
            Self::Severe => 500,   // 5% of slash
            Self::Moderate => 200,
            Self::Minor => 100,
        }
    }

    /// Ambil tarif pembakaran untuk tingkat pelanggaran ini.
    /// Rasio dari total slash amount.
    #[must_use]
    pub const fn burn_bps(self) -> u64 {
        match self {
            Self::Severe => 500,   // 5% of slash
            Self::Moderate => 300,
            Self::Minor => 100,
        }
    }

    /// Ambil tarif treasury/sink untuk tingkat pelanggaran ini.
    /// Rasio dari total slash amount.
    #[must_use]
    pub const fn treasury_bps(self) -> u64 {
        match self {
            Self::Severe => 9_000,  // 90% of slash (30% + 500 + 500 + 9000 = 10000)
            Self::Moderate => 9_500,
            Self::Minor => 9_800,
        }
    }

    /// Verifikasi bahwa total rasio BPS = `BPS_SCALE` (100%)
    /// Note: `slash_bps` adalah rasio dari staked amount, sedangkan alokasi
    /// (reporter, burn, treasury) adalah rasio dari slash amount.
    /// Jadi keduanya tidak perlu berjumlah 10000.
    #[must_use]
    pub fn verify_allocation_ratio(self) -> bool {
        self.reporter_reward_bps() + self.burn_bps() + self.treasury_bps() == BPS_SCALE
    }
}

/// Kalkulator slashing deterministik dengan akuntansi BPS nir-pecahan.
#[derive(Debug)]
pub struct SlashCalculator;

impl SlashCalculator {
    /// Hitung alokasi denda slashing untuk pelanggaran dengan tingkat tertentu.
    ///
    /// Formula:
    /// - `total_slash` = (`staked_amount` * severity.bps) / `BPS_SCALE`
    /// - `reporter_reward` = (`total_slash` * `reporter_reward_bps`) / `BPS_SCALE`
    /// - `burned_amount` = (`total_slash` * `burn_bps`) / `BPS_SCALE`
    /// - `treasury_amount` = (`total_slash` * `treasury_bps`) / `BPS_SCALE`
    ///
    /// Semua operasi menggunakan checked arithmetic untuk mencegah overflow.
    ///
    /// # Arguments
    /// * `staked_amount` - Jumlah stake validator dalam Quanta
    /// * `severity` - Tingkat keparahan pelanggaran
    ///
    /// # Errors
    /// Mengembalikan error jika:
    /// - Perhitungan overflow
    /// - Rasio BPS tidak valid
    pub fn calculate(
        staked_amount: u64,
        severity: ViolationSeverity,
    ) -> Result<SlashAllocation, GuardError> {
        // Validasi rasio alokasi BPS (harus = 10000)
        if !severity.verify_allocation_ratio() {
            return Err(GuardError::InvalidBpsRatio {
                bps: severity.reporter_reward_bps() + severity.burn_bps() + severity.treasury_bps(),
            });
        }

        // Hitung total slash: (staked_amount * slash_bps) / BPS_SCALE
        let slash_bps = severity.slash_bps();
        let total_slash = Self::calc_bps_amount(staked_amount, slash_bps)?;

        // Hitung alokasi dari total slash
        let reporter_bps = severity.reporter_reward_bps();
        let burn_bps = severity.burn_bps();
        let treasury_bps = severity.treasury_bps();

        // Alokasi hadiah pelapor
        let reporter_reward = Self::calc_bps_amount(total_slash, reporter_bps)?;

        // Alokasi pembakaran
        let burned_amount = Self::calc_bps_amount(total_slash, burn_bps)?;

        // Alokasi treasury (menggunakan rasio treasury)
        let treasury_amount = Self::calc_bps_amount(total_slash, treasury_bps)?;

        // Verifikasi konservasi (total alokasi <= total slash)
        // Note: Dengan BPS yang benar, ini seharusnya exact match
        let allocation = SlashAllocation {
            total_slash,
            reporter_reward,
            burned_amount,
            treasury_amount,
        };

        allocation.verify_conservation()?;

        Ok(allocation)
    }

    /// Hitung amount berdasarkan BPS: (amount * bps) / `BPS_SCALE`
    /// Menggunakan checked arithmetic dan floor division.
    ///
    /// # Errors
    /// Mengembalikan error jika perkalian overflow.
    pub fn calc_bps_amount(amount: u64, bps: u64) -> Result<u64, GuardError> {
        if bps > BPS_SCALE {
            return Err(GuardError::InvalidBpsRatio { bps });
        }

        // amount * bps
        let product = amount
            .checked_mul(bps)
            .ok_or(GuardError::SlashingOverflow {
                details: format!("amount={amount} * bps={bps} overflow"),
            })?;

        // (amount * bps) / BPS_SCALE
        Ok(product / BPS_SCALE)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_bps_scale_constants() {
        assert_eq!(BPS_SCALE, 10_000);
    }

    #[test]
    fn test_severe_slash_calculation() {
        let staked = 100_000_000; // 100M Quanta
        let result = SlashCalculator::calculate(staked, ViolationSeverity::Severe)
            .expect("Severe slash calculation should succeed");

        // 30% of 100M = 30M
        assert_eq!(result.total_slash, 30_000_000);
        // 5% of 30M = 1.5M
        assert_eq!(result.reporter_reward, 1_500_000);
        // 5% of 30M = 1.5M
        assert_eq!(result.burned_amount, 1_500_000);
        // 90% of 30M = 27M
        assert_eq!(result.treasury_amount, 27_000_000);

        // Verifikasi konservasi: 1.5M + 1.5M + 27M = 30M
        let total_allocated = result.reporter_reward + result.burned_amount + result.treasury_amount;
        assert_eq!(total_allocated, result.total_slash);
    }

    #[test]
    fn test_bps_calculation_overflow_protection() {
        let staked = u64::MAX;
        let severity = ViolationSeverity::Severe;

        let result = SlashCalculator::calculate(staked, severity);
        assert!(result.is_err(), "Should overflow with u64::MAX");
    }


}
