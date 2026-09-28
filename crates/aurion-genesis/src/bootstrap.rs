use crate::{error::GenesisError, spec::GenesisSpec};
use aurion_core::{Account, Block, BlockHeader, State};
use aurion_ledger::LedgerStore;

pub struct GenesisBootstrap;

impl GenesisBootstrap {
    /// Inisialisasi basis data ledger fisik dengan Blok 0 dan akun Treasury
    pub fn initialize_ledger(
        ledger: &LedgerStore,
        spec: &GenesisSpec,
    ) -> Result<Block, GenesisError> {
        // Guard: tolak jika blok 0 sudah ada (bukan cek latest_height,
        // karena commit_block tidak pernah menulis height 0 ke metadata).
        if ledger.get_block_by_height(0)?.is_some() {
            let h = ledger.get_latest_height()?;
            return Err(GenesisError::AlreadyInitialized(h));
        }
        // 1. State FSM + suntik 66jt AUR ke Treasury (Nonce 0)
        let mut state = State::new();
        let total_quanta = spec.total_supply_quanta()?;
        state.insert_account(spec.treasury_account, Account::new(total_quanta, 0));
        // 2. State root kanonikal Blok 0
        let genesis_state_root = state.compute_state_root();
        // 3. Blok 0 (prev = nol mutlak, 0 tx)
        let genesis_block = Block {
            header: BlockHeader {
                height: 0,
                prev_hash: [0u8; 32],
                state_root: genesis_state_root,
                tx_count: 0,
            },
            transactions: Vec::new(),
        };
        // 4. Tulis ke redb via jalur yang sama dengan blok normal
        // (Block::execute memvalidasi state_root lalu commit menulis height+index).
        ledger.commit_block(&genesis_block, &mut state)?;
        Ok(genesis_block)
    }
}
