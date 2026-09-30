use crate::cache::TransactionalCache;
use crate::error::ExecutionError;
use crate::keeper::{AccountKeeper, Keeper};
use crate::store_key::StoreKey;
use aurion_core::types::{Quanta, TREASURY_GENESIS_QUANTA};

const CYCLE_KEY: &[u8] = b"cycle_index";
const TOTAL_MINTED_KEY: &[u8] = b"total_minted";
const LAST_REPLENISH_EVENT_KEY: &[u8] = b"aurion:replenish:last_cycle";

/// Mesin pencetakan pasokan siklus (Perpetual Replenishment Engine).
///
/// Kebijakan: bila saldo akun Treasury mencapai nol mutlak pada titik eksekusi
/// blok, engine mengkreditkan kembali tepat `TREASURY_GENESIS_QUANTA`
/// (66.000.000 AUR) dan menaikkan `cycle_index` sistem. Seluruh angka moneter
/// memakai `Quanta` (`u128`); tidak ada jalur `.unwrap()`/`.expect()` di jalur
/// produksi.
#[derive(Debug)]
pub struct ReplenishmentEngine {
    accounts: AccountKeeper,
    params_store: StoreKey,
}

impl ReplenishmentEngine {
    /// Membangun engine dengan partisi akses parameter sistem `"replenish"`.
    ///
    /// # Errors
    /// Mengembalikan `InvalidStoreKey` bila label namespace tidak valid.
    pub fn new() -> Result<Self, ExecutionError> {
        Ok(Self {
            accounts: AccountKeeper::new()?,
            params_store: StoreKey::new("replenish")?,
        })
    }

    fn read_quanta(
        &self,
        cache: &mut TransactionalCache,
        user_key: &[u8],
    ) -> Result<Quanta, ExecutionError> {
        let qualified = self.params_store.qualify(user_key);
        match cache.get(&qualified)? {
            Some(bytes) => {
                let slice: [u8; 16] =
                    bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| ExecutionError::MalformedState {
                            reason: "Format parameter Quanta rusak (bukan 16-byte u128)",
                        })?;
                Ok(u128::from_be_bytes(slice))
            }
            None => Ok(0),
        }
    }

    /// Memeriksa saldo Treasury dan, bila mencapai nol, mencetak satu siklus
    /// pasokan (`66_000_000_000_000_000_000` Quanta diparameterkan sebagai
    /// `TREASURY_GENESIS_QUANTA`).
    ///
    /// Mengembalikan `Ok(Some(cycle_baru))` bila replenishment dieksekusi dan
    /// `Ok(None)` bila saldo Treasury masih positif (tidak ada pencetakan).
    ///
    /// # Errors
    /// Mengembalikan `ArithmeticOverflow` bila `cycle_index` atau `total_minted`
    /// meluap, serta `MalformedState` bila format byte state rusak.
    pub fn check_and_replenish(
        &self,
        cache: &mut TransactionalCache,
        treasury: &[u8; 32],
    ) -> Result<Option<u64>, ExecutionError> {
        let balance_key = self
            .accounts
            .store_key()
            .qualify(&self.accounts.balance_key(treasury));

        let treasury_balance =
            match cache.get(&balance_key)? {
                Some(bytes) => {
                    let slice: [u8; 16] = bytes.as_slice().try_into().map_err(|_| {
                        ExecutionError::MalformedState {
                            reason: "Format saldo Treasury rusak (bukan 16-byte u128)",
                        }
                    })?;
                    u128::from_be_bytes(slice)
                }
                None => 0,
            };

        if treasury_balance > 0 {
            return Ok(None);
        }

        let cycle =
            match cache.get(&self.params_store.qualify(CYCLE_KEY))? {
                Some(bytes) => {
                    let slice: [u8; 8] = bytes.as_slice().try_into().map_err(|_| {
                        ExecutionError::MalformedState {
                            reason: "Format cycle_index rusak (bukan 8-byte u64)",
                        }
                    })?;
                    u64::from_be_bytes(slice)
                }
                None => 0,
            };
        let new_cycle = cycle
            .checked_add(1)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        let total_minted = self.read_quanta(cache, TOTAL_MINTED_KEY)?;
        let new_total_minted = total_minted
            .checked_add(TREASURY_GENESIS_QUANTA)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        cache.set(balance_key, TREASURY_GENESIS_QUANTA.to_be_bytes().to_vec())?;
        cache.set(
            self.params_store.qualify(CYCLE_KEY),
            new_cycle.to_be_bytes().to_vec(),
        )?;
        cache.set(
            self.params_store.qualify(TOTAL_MINTED_KEY),
            new_total_minted.to_be_bytes().to_vec(),
        )?;
        cache.set(
            self.params_store.qualify(LAST_REPLENISH_EVENT_KEY),
            new_cycle.to_be_bytes().to_vec(),
        )?;

        Ok(Some(new_cycle))
    }
}
