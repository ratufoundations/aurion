use crate::{error::ExecutionError, state::State, transaction::Transaction};
use aurion_criptografi::{Hash256, PublicKeyBytes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    pub height: u64,
    pub prev_hash: Hash256,
    pub state_root: Hash256,
    pub tx_count: u32,
    pub timestamp: u64,
    pub proposer: PublicKeyBytes,
}

impl BlockHeader {
    #[must_use]
    pub fn hash(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_BLOCK_HEADER_V1");
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.prev_hash);
        hasher.update(&self.state_root);
        hasher.update(&self.tx_count.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(&self.proposer);
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Eksekusi blok secara atomik dan validasi jumlah transaksi serta State Root.
    ///
    /// Semua transaksi diterapkan pada salinan state. State aktif hanya diganti jika
    /// seluruh transaksi dan root akhir valid.
    ///
    /// # Errors
    /// Mengembalikan error bila jumlah transaksi, transaksi, atau State Root tidak valid.
    pub fn execute(&self, state: &mut State) -> Result<(), ExecutionError> {
        let tx_count = u32::try_from(self.transactions.len())
            .map_err(|_| ExecutionError::InvalidTransactionCount)?;
        if self.header.tx_count != tx_count {
            return Err(ExecutionError::InvalidTransactionCount);
        }
        tracing::debug!(
            height = self.header.height,
            txs = self.header.tx_count,
            "Eksekusi blok dimulai"
        );
        let mut candidate_state = state.clone();
        for tx in &self.transactions {
            candidate_state.apply_transaction_with_proposer(tx, self.header.proposer)?;
        }
        let computed_root = candidate_state.compute_state_root();
        if computed_root != self.header.state_root {
            tracing::error!(height = self.header.height, "State root blok tidak cocok");
            return Err(ExecutionError::StateRootMismatch);
        }
        *state = candidate_state;
        Ok(())
    }

    /// Validate chain linkage against a known parent and atomically execute the block.
    ///
    /// # Errors
    /// Returns an error for height overflow, a non-sequential height, a parent hash
    /// mismatch, or any transaction/state-root validation failure.
    pub fn execute_with_parent(
        &self,
        state: &mut State,
        parent: &BlockHeader,
    ) -> Result<(), ExecutionError> {
        let expected_height = parent
            .height
            .checked_add(1)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        if self.header.height != expected_height {
            return Err(ExecutionError::NonSequentialHeight {
                expected: expected_height,
                got: self.header.height,
            });
        }
        if self.header.prev_hash != parent.hash() {
            return Err(ExecutionError::InvalidParentHash);
        }
        if self.header.timestamp <= parent.timestamp {
            return Err(ExecutionError::TimestampNotMonotonic);
        }
        if state.compute_state_root() != parent.state_root {
            return Err(ExecutionError::ParentStateRootMismatch);
        }
        self.execute(state)
    }
}
