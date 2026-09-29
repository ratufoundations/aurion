use crate::error::ExecutionError;

pub const FUEL_COST_READ: u64 = 5;
pub const FUEL_COST_WRITE: u64 = 20;
pub const FUEL_COST_ACTION_BASE: u64 = 50;

/// Meter komputasi deterministik murni bilangan bulat (u64).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuelMeter {
    limit: u64,
    consumed: u64,
}

impl FuelMeter {
    #[must_use]
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }

    pub fn charge(&mut self, amount: u64) -> Result<(), ExecutionError> {
        let remaining = self.remaining();
        if remaining < amount {
            return Err(ExecutionError::OutOfFuel {
                required: amount,
                remaining,
            });
        }

        self.consumed = self
            .consumed
            .checked_add(amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        Ok(())
    }

    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    #[must_use]
    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    #[must_use]
    pub fn limit(&self) -> u64 {
        self.limit
    }
}
