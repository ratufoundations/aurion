use crate::error::AccountError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendingPolicy {
    pub max_per_tx: u64,
    pub daily_quota: u64,
    pub current_spent: u64,
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
    pub fn check_and_update(&mut self, amount: u64, current_time: u64) -> Result<(), AccountError> {
        if current_time.saturating_sub(self.last_reset_time) >= 86400 {
            self.current_spent = 0;
            self.last_reset_time = current_time;
        }
        if amount > self.max_per_tx {
            return Err(AccountError::ExceedsPerTxLimit {
                amount,
                limit: self.max_per_tx,
            });
        }
        let new_spent = self.current_spent.saturating_add(amount);
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
