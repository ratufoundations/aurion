use crate::{error::ExecutionError, state::State, transaction::Transaction};
use aurion_criptografi::Hash256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    pub height: u64,
    pub prev_hash: Hash256,
    pub state_root: Hash256,
    pub tx_count: u32,
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
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Eksekusi sekumpulan transaksi dalam blok dan validasi State Root akhir.
    ///
    /// # Errors
    /// Mengembalikan error bila transaksi ditolak atau state root akhir tidak cocok.
    pub fn execute(&self, state: &mut State) -> Result<(), ExecutionError> {
        tracing::debug!(
            height = self.header.height,
            txs = self.header.tx_count,
            "Eksekusi blok dimulai"
        );
        for tx in &self.transactions {
            state.apply_transaction(tx)?;
        }

        let computed_root = state.compute_state_root();
        if computed_root != self.header.state_root {
            tracing::error!(height = self.header.height, "State root blok tidak cocok");
            return Err(ExecutionError::StateRootMismatch);
        }

        Ok(())
    }
}
