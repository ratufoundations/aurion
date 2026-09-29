use crate::error::VmError;

#[derive(Debug)]
pub struct GasMeter {
    pub limit: u64,
    pub consumed: u64,
}

impl GasMeter {
    #[must_use]
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }

    /// Potong bahan bakar (fuel). Kembalikan `VmError::OutOfGas` jika kuota habis.
    ///
    /// # Errors
    ///
    /// Mengembalikan `VmError::OutOfGas` jika konsumsi melewati batas gas.
    pub fn consume(&mut self, amount: u64) -> Result<(), VmError> {
        let new_consumed = self.consumed.saturating_add(amount);
        if new_consumed > self.limit {
            self.consumed = self.limit;
            return Err(VmError::OutOfGas);
        }
        self.consumed = new_consumed;
        Ok(())
    }

    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }
}
