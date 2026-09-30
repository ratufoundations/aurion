use crate::action::{Action, ActionBatch};
use crate::cache::{TransactionalCache, WriteSet};
use crate::capability::CapabilityRegistry;
use crate::error::ExecutionError;
use crate::fuel::FUEL_COST_ACTION_BASE;
use crate::keeper::{AccountKeeper, GovernanceKeeper, Keeper, StakingKeeper, ValidatorKeeper};
use crate::state_root::compute_state_root;
use crate::store_key::NamespaceStore;
use aurion_core::types::Quanta;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPolicy {
    pub fuel_limit: u64,
    pub max_actions_per_tx: usize,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            fuel_limit: 100_000,
            max_actions_per_tx: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionContext {
    pub block_height: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionOutcome {
    pub height: u64,
    pub fuel_consumed: u64,
    pub delta: WriteSet,
    pub state_root: [u8; 32],
}

/// Mesin orkestrator eksekusi transaksi berbasis Capability-Keeper.
#[derive(Debug)]
pub struct ExecutionEngine {
    committed: BTreeMap<Vec<u8>, Vec<u8>>,
    registry: CapabilityRegistry,
    policy: ExecutionPolicy,
    pub accounts: AccountKeeper,
    pub staking: StakingKeeper,
    pub governance: GovernanceKeeper,
    pub validators: ValidatorKeeper,
}

impl ExecutionEngine {
    pub fn new(policy: ExecutionPolicy, salt: [u8; 32]) -> Result<Self, ExecutionError> {
        Ok(Self {
            committed: BTreeMap::new(),
            registry: CapabilityRegistry::new(salt),
            policy,
            accounts: AccountKeeper::new()?,
            staking: StakingKeeper::new()?,
            governance: GovernanceKeeper::new()?,
            validators: ValidatorKeeper::new()?,
        })
    }

    pub fn registry_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.registry
    }

    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Kueri saldo akun langsung dari state terkonfirmasi (CQRS Read Path).
    /// Read-only, tidak mengonsumsi fuel transaksi, dan bebas alokasi cache kotor.
    pub fn query_balance(&self, account: &[u8; 32]) -> Result<Quanta, ExecutionError> {
        let raw_key = self.accounts.balance_key(account);
        let qualified = self.accounts.store_key().qualify(&raw_key);

        match self.committed.get(&qualified) {
            Some(bytes) => {
                let slice: [u8; 16] =
                    bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| ExecutionError::MalformedState {
                            reason: "Format byte saldo tidak valid (bukan 16 byte u128)",
                        })?;
                Ok(u128::from_be_bytes(slice))
            }
            None => Ok(0),
        }
    }

    /// Kueri metadata akun (CQRS Read Path).
    pub fn query_metadata(
        &self,
        account: &[u8; 32],
        key: &str,
    ) -> Result<Option<String>, ExecutionError> {
        let raw_key = self.accounts.metadata_key(account, key);
        let qualified = self.accounts.store_key().qualify(&raw_key);

        match self.committed.get(&qualified) {
            Some(bytes) => {
                let s = String::from_utf8(bytes.clone()).map_err(|_| {
                    ExecutionError::MalformedState {
                        reason: "Format metadata bukan string UTF-8 valid",
                    }
                })?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }

    /// Mengambil snapshot committed state untuk kebutuhan replay atau audit.
    #[must_use]
    pub fn committed_snapshot(&self) -> BTreeMap<Vec<u8>, Vec<u8>> {
        self.committed.clone()
    }

    #[must_use]
    pub fn state_root(&self) -> [u8; 32] {
        compute_state_root(&self.committed)
    }

    pub fn set_balance_genesis(
        &mut self,
        account: &[u8; 32],
        amount: Quanta,
    ) -> Result<(), ExecutionError> {
        let raw_key = self.accounts.balance_key(account);
        let qualified = self.accounts.store_key().qualify(&raw_key);
        self.committed
            .insert(qualified, amount.to_be_bytes().to_vec());
        Ok(())
    }

    /// Mengeksekusi satu batch transaksi dalam TransactionalCache.
    /// Menjamin atomisitas mutlak: commit jika berhasil, rollback jika gagal.
    pub fn execute(
        &mut self,
        ctx: &ExecutionContext,
        batch: &ActionBatch,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        if batch.len() > self.policy.max_actions_per_tx {
            return Err(ExecutionError::TooManyActions {
                count: batch.len(),
                limit: self.policy.max_actions_per_tx,
            });
        }

        let mut cache = TransactionalCache::new(self.committed.clone(), self.policy.fuel_limit);

        // Eksekusi aksi berurutan
        for action in batch.actions() {
            cache.charge_fuel(FUEL_COST_ACTION_BASE)?;

            if let Err(err) = self.dispatch_action(&mut cache, ctx, action) {
                // OutOfFuel dan CapabilityMissing mengalir langsung tanpa dibungkus
                return match err {
                    ExecutionError::OutOfFuel { .. } | ExecutionError::CapabilityMissing { .. } => {
                        Err(err)
                    }
                    _ => Err(ExecutionError::ExecutionReverted {
                        cause: Box::new(err),
                    }),
                };
            }
        }

        let fuel_consumed = cache.fuel_consumed();
        let (delta, new_committed) = cache.commit();
        self.committed = new_committed;
        let new_root = compute_state_root(&self.committed);

        Ok(ExecutionOutcome {
            height: ctx.block_height,
            fuel_consumed,
            delta,
            state_root: new_root,
        })
    }

    fn dispatch_action(
        &self,
        cache: &mut TransactionalCache,
        ctx: &ExecutionContext,
        action: &Action,
    ) -> Result<(), ExecutionError> {
        match action {
            Action::Transfer { from, to, amount } => {
                let mut store = NamespaceStore::new(self.accounts.store_key().clone(), cache);
                self.accounts.transfer(&mut store, from, to, *amount)
            }
            Action::LockStake { staker, amount } => {
                let cap = StakingKeeper::lock_capability();
                let staking_id = self.staking.module_id();
                let handle = self.registry.issue_handle(&cap, &staking_id)?;

                // First, execute lock_balance on account store
                {
                    let mut account_store =
                        NamespaceStore::new(self.accounts.store_key().clone(), cache);
                    self.accounts.lock_balance(
                        &mut account_store,
                        &self.registry,
                        &handle,
                        ctx.block_height,
                        staker,
                        *amount,
                    )?;
                }

                // Then, update staking store
                {
                    let mut staking_store =
                        NamespaceStore::new(self.staking.store_key().clone(), cache);

                    // Manually replicate the staking logic
                    let mut key = Vec::with_capacity(6 + 32);
                    key.extend_from_slice(b"stake:");
                    key.extend_from_slice(staker);

                    let current_stake = match staking_store.get(&key)? {
                        Some(bytes) => {
                            let slice: [u8; 16] = bytes.as_slice().try_into().map_err(|_| {
                                ExecutionError::MalformedState {
                                    reason: "Format stake rusak (bukan 16-byte u128)",
                                }
                            })?;
                            u128::from_be_bytes(slice)
                        }
                        None => 0,
                    };

                    let new_stake = current_stake
                        .checked_add(*amount)
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                    staking_store.set(&key, &new_stake.to_be_bytes())?;
                }

                Ok(())
            }
            Action::UpdateMetadata {
                account,
                key,
                value,
            } => {
                let mut store = NamespaceStore::new(self.accounts.store_key().clone(), cache);
                self.accounts.set_metadata(&mut store, account, key, value)
            }
            Action::SetProposal {
                proposal_id, title, ..
            } => {
                let mut store = NamespaceStore::new(self.governance.store_key().clone(), cache);
                self.governance
                    .set_proposal(&mut store, *proposal_id, title)
            }
            Action::FailExplicitly { reason } => Err(ExecutionError::ExplicitFailure {
                reason: reason.clone(),
            }),
        }
    }
}
