use crate::error::AccountError;
use aurion_core::types::Quanta;

/// Ambang kuorum baku protokol dalam persen bilangan bulat (`u64`).
pub const QUORUM_THRESHOLD_PERCENT: u64 = 67;

/// Jeda reset kuota harian dalam detik (bilangan bulat, tanpa fraksi waktu).
pub const DAILY_QUOTA_WINDOW_SECONDS: u64 = 86_400;

/// Pemeriksaan kuorum berbasis bilangan bulat murni.
///
/// Mengembalikan `true` bila `accumulated * 100 >= total * threshold_percent`.
/// Seluruh perkalian memakai `checked_mul`: bila representasi `u64` tidak cukup,
/// hasilnya adalah `false` (tanpa panic dan tanpa overflow yang membungkam batas).
#[must_use]
pub fn meets_integer_quorum(accumulated: u64, total: u64, threshold_percent: u64) -> bool {
    if total == 0 || threshold_percent == 0 || threshold_percent > 100 {
        return false;
    }
    match (
        accumulated.checked_mul(100),
        total.checked_mul(threshold_percent),
    ) {
        (Some(lhs), Some(rhs)) => lhs >= rhs,
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendingPolicy {
    pub max_per_tx: Quanta,
    pub daily_quota: Quanta,
    pub current_spent: Quanta,
    pub last_reset_time: u64,
}

impl Default for SpendingPolicy {
    fn default() -> Self {
        Self {
            max_per_tx: 50_000,
            daily_quota: 200_000,
            current_spent: 0,
            last_reset_time: 0,
        }
    }
}

impl SpendingPolicy {
    /// Sisa kuota harian dalam Quanta (bilangan bulat; nol bila kuota terlampaui).
    #[must_use]
    pub fn remaining_quota(&self) -> Quanta {
        self.daily_quota.saturating_sub(self.current_spent)
    }

    /// Periksa batas transaksi/kuota dan mutakhirkan pemakaian.
    ///
    /// Akumulasi pemakaian memakai `checked_add`, sehingga luapan `u128` selalu
    /// menjadi error bertipe dan tidak pernah membungkam batas kuota.
    ///
    /// # Errors
    /// Mengembalikan `AccountError::ExceedsPerTxLimit` bila melampaui batas per
    /// transaksi, `AccountError::ExceedsDailyQuota` bila kuota harian terlampaui,
    /// dan `AccountError::ArithmeticOverflow` bila akumulasi meluap.
    pub fn check_and_update(
        &mut self,
        amount: Quanta,
        current_time: u64,
    ) -> Result<(), AccountError> {
        if current_time.saturating_sub(self.last_reset_time) >= DAILY_QUOTA_WINDOW_SECONDS {
            self.current_spent = 0;
            self.last_reset_time = current_time;
        }
        if amount > self.max_per_tx {
            return Err(AccountError::ExceedsPerTxLimit {
                amount,
                limit: self.max_per_tx,
            });
        }
        let new_spent = self
            .current_spent
            .checked_add(amount)
            .ok_or(AccountError::ArithmeticOverflow)?;
        if new_spent > self.daily_quota {
            return Err(AccountError::ExceedsDailyQuota {
                spent: new_spent,
                limit: self.daily_quota,
            });
        }
        self.current_spent = new_spent;
        Ok(())
    }
}
