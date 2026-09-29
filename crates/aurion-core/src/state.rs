use crate::{
    account::Account,
    error::ExecutionError,
    module::{StateReader, StateWriter},
    transaction::Transaction,
};
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    // Gunakan BTreeMap untuk memastikan iterasi selalu terurut (deterministik)
    accounts: BTreeMap<PublicKeyBytes, Account>,
    module_data: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl State {
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
            module_data: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn get_account(&self, pubkey: &PublicKeyBytes) -> Option<&Account> {
        self.accounts.get(pubkey)
    }

    pub fn insert_account(&mut self, pubkey: PublicKeyBytes, account: Account) {
        self.accounts.insert(pubkey, account);
    }

    #[must_use]
    pub fn accounts(&self) -> &BTreeMap<PublicKeyBytes, Account> {
        &self.accounts
    }

    /// Transisi status atomik: `S(t+1) = f(S_t, Tx)`.
    ///
    /// # Errors
    /// Mengembalikan error bila tanda tangan, nonce, saldo, atau aritmatika tidak valid.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), ExecutionError> {
        if tx.sender == tx.recipient {
            tracing::error!(account = ?tx.sender, "Percobaan transfer ke akun sendiri");
            return Err(ExecutionError::SelfTransferForbidden);
        }

        // 1. Verifikasi tanda tangan digital
        if let Err(error) = tx.verify_signature() {
            tracing::error!(account = ?tx.sender, error = %error, "Tanda tangan transaksi ditolak");
            return Err(error);
        }

        // 2. Baca status sender
        let sender_acc = self
            .accounts
            .get(&tx.sender)
            .copied()
            .ok_or(ExecutionError::AccountNotFound(tx.sender))?;

        // 3. Validasi Nonce
        if tx.nonce != sender_acc.nonce {
            return Err(ExecutionError::InvalidNonce {
                expected: sender_acc.nonce,
                got: tx.nonce,
            });
        }

        // 4. Validasi Saldo Sender
        if sender_acc.balance < tx.amount {
            tracing::error!(account = ?tx.sender, available = sender_acc.balance, required = tx.amount, "Percobaan double-spend / saldo tidak mencukupi");
            return Err(ExecutionError::InsufficientBalance {
                available: sender_acc.balance,
                required: tx.amount,
            });
        }

        // 5. Baca status recipient (buat baru jika belum terdaftar di state)
        let recipient_acc = self
            .accounts
            .get(&tx.recipient)
            .copied()
            .unwrap_or(Account::new(0, 0));

        // 6. Hitung saldo baru dengan pengecekan overflow
        let new_sender_balance = sender_acc
            .balance
            .checked_sub(tx.amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        let new_sender_nonce = sender_acc
            .nonce
            .checked_add(1)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        let new_recipient_balance = recipient_acc
            .balance
            .checked_add(tx.amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;

        // 7. Commit pembaruan ke in-memory state
        self.accounts.insert(
            tx.sender,
            Account::new(new_sender_balance, new_sender_nonce),
        );
        self.accounts.insert(
            tx.recipient,
            Account::new(new_recipient_balance, recipient_acc.nonce),
        );
        tracing::debug!(account = ?tx.sender, new_balance = new_sender_balance, "State akun dimutasi");

        Ok(())
    }

    /// Menghitung State Root deterministik 32 bita dari seluruh akun yang terurut
    #[must_use]
    pub fn compute_state_root(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_STATE_ROOT_V1");

        for (pubkey, account) in &self.accounts {
            let acc_hash = account.hash(pubkey);
            hasher.update(&acc_hash);
        }
        hasher.update(b"AURION_MODULE_STATE_V1");
        for (key, value) in &self.module_data {
            hasher.update(&u64::try_from(key.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(key);
            hasher.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(value);
        }

        *hasher.finalize().as_bytes()
    }
}

impl StateReader for State {
    fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.module_data.get(key).map(Vec::as_slice)
    }
}

impl StateWriter for State {
    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.module_data.insert(key.to_vec(), value.to_vec());
    }

    fn remove(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.module_data.remove(key)
    }
}
