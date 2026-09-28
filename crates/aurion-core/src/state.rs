use crate::{account::Account, error::ExecutionError, transaction::Transaction};
use aurion_criptografi::{Hash256, PublicKeyBytes};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    // Gunakan BTreeMap untuk memastikan iterasi selalu terurut (deterministik)
    accounts: BTreeMap<PublicKeyBytes, Account>,
}

impl State {
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    pub fn get_account(&self, pubkey: &PublicKeyBytes) -> Option<&Account> {
        self.accounts.get(pubkey)
    }

    pub fn insert_account(&mut self, pubkey: PublicKeyBytes, account: Account) {
        self.accounts.insert(pubkey, account);
    }

    pub fn accounts(&self) -> &BTreeMap<PublicKeyBytes, Account> {
        &self.accounts
    }

    /// Transisi status atomik: S(t+1) = f(S_t, Tx)
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), ExecutionError> {
        if tx.sender == tx.recipient {
            return Err(ExecutionError::SelfTransferForbidden);
        }

        // 1. Verifikasi tanda tangan digital
        tx.verify_signature()?;

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

        Ok(())
    }

    /// Menghitung State Root deterministik 32 bita dari seluruh akun yang terurut
    pub fn compute_state_root(&self) -> Hash256 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_STATE_ROOT_V1");

        for (pubkey, account) in &self.accounts {
            let acc_hash = account.hash(pubkey);
            hasher.update(&acc_hash);
        }

        *hasher.finalize().as_bytes()
    }
}
