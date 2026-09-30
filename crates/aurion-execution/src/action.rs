//! Modul aksi bisnis yang dapat dieksekusi di mesin eksekusi Aurion.

use aurion_core::types::Quanta;

/// Jenis-jenis aksi bisnis yang dapat dieksekusi di modul Aurion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Transfer {
        from: [u8; 32],
        to: [u8; 32],
        amount: Quanta,
    },
    LockStake {
        staker: [u8; 32],
        amount: Quanta,
    },
    UpdateMetadata {
        account: [u8; 32],
        key: String,
        value: String,
    },
    SetProposal {
        proposer: [u8; 32],
        proposal_id: u64,
        title: String,
    },
    FailExplicitly {
        reason: String,
    },
}

impl Action {
    #[must_use]
    pub fn action_code(&self) -> u16 {
        match self {
            Self::Transfer { .. } => 1,
            Self::LockStake { .. } => 2,
            Self::UpdateMetadata { .. } => 3,
            Self::SetProposal { .. } => 4,
            Self::FailExplicitly { .. } => 99,
        }
    }
}

/// Sekumpulan aksi yang dikemas dalam satu transaksi atomik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBatch {
    actions: Vec<Action>,
}

impl ActionBatch {
    #[must_use]
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    #[must_use]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

impl Default for ActionBatch {
    fn default() -> Self {
        Self::new()
    }
}
