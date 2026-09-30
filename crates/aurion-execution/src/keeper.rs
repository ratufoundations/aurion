use crate::capability::{Capability, CapabilityHandle, CapabilityRegistry, ModuleId};
use crate::error::ExecutionError;
use crate::store_key::{NamespaceStore, StoreKey};
use aurion_core::types::Quanta;

pub trait Keeper {
    fn store_key(&self) -> &StoreKey;
    fn module_id(&self) -> ModuleId;
    fn name(&self) -> &'static str;
}

/// Keeper Akun untuk mengelola saldo dan metadata.
#[derive(Debug)]
pub struct AccountKeeper {
    store_key: StoreKey,
}

impl AccountKeeper {
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            store_key: StoreKey::new("account")?,
        })
    }

    #[must_use]
    pub fn balance_key(&self, account: &[u8; 32]) -> Vec<u8> {
        let mut k = Vec::with_capacity(4 + 32);
        k.extend_from_slice(b"bal:");
        k.extend_from_slice(account);
        k
    }

    #[must_use]
    pub fn metadata_key(&self, account: &[u8; 32], key: &str) -> Vec<u8> {
        let mut k = Vec::with_capacity(5 + 32 + key.len());
        k.extend_from_slice(b"meta:");
        k.extend_from_slice(account);
        k.extend_from_slice(b":");
        k.extend_from_slice(key.as_bytes());
        k
    }

    pub fn balance(
        &self,
        store: &mut NamespaceStore<'_>,
        account: &[u8; 32],
    ) -> Result<Quanta, ExecutionError> {
        let key = self.balance_key(account);
        match store.get(&key)? {
            Some(bytes) => {
                let slice: [u8; 16] =
                    bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| ExecutionError::MalformedState {
                            reason: "Format saldo rusak (bukan 16-byte u128)",
                        })?;
                Ok(u128::from_be_bytes(slice))
            }
            None => Ok(0),
        }
    }

    pub fn set_balance(
        &self,
        store: &mut NamespaceStore<'_>,
        account: &[u8; 32],
        balance: Quanta,
    ) -> Result<(), ExecutionError> {
        let key = self.balance_key(account);
        store.set(&key, &balance.to_be_bytes())
    }

    pub fn transfer(
        &self,
        store: &mut NamespaceStore<'_>,
        from: &[u8; 32],
        to: &[u8; 32],
        amount: Quanta,
    ) -> Result<(), ExecutionError> {
        if from == to {
            return Err(ExecutionError::SelfTransferForbidden);
        }
        // Blacklist alamat burn terlarang/mati
        if to == &[0xEE; 32] {
            return Err(ExecutionError::ForbiddenRecipient(*to));
        }

        let from_bal = self.balance(store, from)?;
        let new_from_bal =
            from_bal
                .checked_sub(amount)
                .ok_or(ExecutionError::InsufficientBalance {
                    available: from_bal,
                    required: amount,
                })?;

        let to_bal = self.balance(store, to)?;
        let new_to_bal = to_bal
            .checked_add(amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        self.set_balance(store, from, new_from_bal)?;
        self.set_balance(store, to, new_to_bal)?;
        Ok(())
    }

    /// Metode berhak istimewa: Mengunci saldo akun (wajib menyertakan kapabilitas sah).
    pub fn lock_balance(
        &self,
        store: &mut NamespaceStore<'_>,
        registry: &CapabilityRegistry,
        handle: &CapabilityHandle,
        current_block: u64,
        owner: &[u8; 32],
        amount: Quanta,
    ) -> Result<(), ExecutionError> {
        registry.authorize(handle, current_block)?;

        let current_bal = self.balance(store, owner)?;
        let new_bal =
            current_bal
                .checked_sub(amount)
                .ok_or(ExecutionError::InsufficientBalance {
                    available: current_bal,
                    required: amount,
                })?;

        self.set_balance(store, owner, new_bal)?;
        Ok(())
    }

    pub fn set_metadata(
        &self,
        store: &mut NamespaceStore<'_>,
        account: &[u8; 32],
        key: &str,
        value: &str,
    ) -> Result<(), ExecutionError> {
        let k = self.metadata_key(account, key);
        store.set(&k, value.as_bytes())
    }
}

impl Keeper for AccountKeeper {
    fn store_key(&self) -> &StoreKey {
        &self.store_key
    }
    fn module_id(&self) -> ModuleId {
        ModuleId::from_store_key(&self.store_key)
    }
    fn name(&self) -> &'static str {
        "account"
    }
}

/// Keeper Staking yang membutuhkan kapabilitas lock_balance milik AccountKeeper.
#[derive(Debug)]
pub struct StakingKeeper {
    store_key: StoreKey,
}

impl StakingKeeper {
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            store_key: StoreKey::new("staking")?,
        })
    }

    #[must_use]
    pub fn lock_capability() -> Capability {
        Capability::new("lock_balance")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lock_stake(
        &self,
        staking_store: &mut NamespaceStore<'_>,
        account_keeper: &AccountKeeper,
        account_store: &mut NamespaceStore<'_>,
        registry: &CapabilityRegistry,
        handle: &CapabilityHandle,
        current_block: u64,
        staker: &[u8; 32],
        amount: Quanta,
    ) -> Result<(), ExecutionError> {
        // Panggil metode berhak istimewa di AccountKeeper menggunakan handle
        account_keeper.lock_balance(
            account_store,
            registry,
            handle,
            current_block,
            staker,
            amount,
        )?;

        // Catat stake di partisi staking sendiri
        let mut key = Vec::with_capacity(6 + 32);
        key.extend_from_slice(b"stake:");
        key.extend_from_slice(staker);

        let current_stake =
            match staking_store.get(&key)? {
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
            .checked_add(amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        staking_store.set(&key, &new_stake.to_be_bytes())?;
        Ok(())
    }
}

impl Keeper for StakingKeeper {
    fn store_key(&self) -> &StoreKey {
        &self.store_key
    }
    fn module_id(&self) -> ModuleId {
        ModuleId::from_store_key(&self.store_key)
    }
    fn name(&self) -> &'static str {
        "staking"
    }
}

/// Modul liar tanpa izin untuk memvalidasi proteksi kapabilitas.
#[derive(Debug)]
pub struct ArbitraryModule {
    store_key: StoreKey,
}

impl ArbitraryModule {
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            store_key: StoreKey::new("arbitrary")?,
        })
    }
}

impl Keeper for ArbitraryModule {
    fn store_key(&self) -> &StoreKey {
        &self.store_key
    }
    fn module_id(&self) -> ModuleId {
        ModuleId::from_store_key(&self.store_key)
    }
    fn name(&self) -> &'static str {
        "arbitrary"
    }
}

/// Keeper Tata Kelola.
#[derive(Debug)]
pub struct GovernanceKeeper {
    store_key: StoreKey,
}

impl GovernanceKeeper {
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            store_key: StoreKey::new("governance")?,
        })
    }

    pub fn set_proposal(
        &self,
        store: &mut NamespaceStore<'_>,
        proposal_id: u64,
        title: &str,
    ) -> Result<(), ExecutionError> {
        let mut key = Vec::with_capacity(5 + 8);
        key.extend_from_slice(b"prop:");
        key.extend_from_slice(&proposal_id.to_be_bytes());
        store.set(&key, title.as_bytes())
    }
}

impl Keeper for GovernanceKeeper {
    fn store_key(&self) -> &StoreKey {
        &self.store_key
    }
    fn module_id(&self) -> ModuleId {
        ModuleId::from_store_key(&self.store_key)
    }
    fn name(&self) -> &'static str {
        "governance"
    }
}

/// Keeper Validator.
#[derive(Debug)]
pub struct ValidatorKeeper {
    store_key: StoreKey,
}

impl ValidatorKeeper {
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            store_key: StoreKey::new("validator")?,
        })
    }
}

impl Keeper for ValidatorKeeper {
    fn store_key(&self) -> &StoreKey {
        &self.store_key
    }
    fn module_id(&self) -> ModuleId {
        ModuleId::from_store_key(&self.store_key)
    }
    fn name(&self) -> &'static str {
        "validator"
    }
}
