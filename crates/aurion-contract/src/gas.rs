use crate::error::VmError;

pub struct GasMeter {
    pub limit: u64,
    pub consumed: u64,
}

impl GasMeter {
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }

    /// Potong bahan bakar (fuel). Kembalikan VmError::OutOfGas jika kuota habis.
    #[inline(always)]
    pub fn consume(&mut self, amount: u64) -> Result<(), VmError> {
        let new_consumed = self.consumed.saturating_add(amount);
        if new_consumed > self.limit {
            self.consumed = self.limit;
            return Err(VmError::OutOfGas);
        }
        self.consumed = new_consumed;
        Ok(())
    }

    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }
}
