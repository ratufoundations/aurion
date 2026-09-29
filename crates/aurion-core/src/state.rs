use crate::{
    account::Account,
    error::ExecutionError,
    module::{StateReader, StateWriter},
    transaction::Transaction,
};
use aurion_criptografi::{Hash256, PublicKeyBytes};

pub const PROTOCOL_FEE_SINK: PublicKeyBytes = [0; 32];
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

    /// Apply one transaction and route its fee to the protocol sink.
    ///
    /// # Errors
    /// Returns a typed error if signature, nonce, fee, solvency, or arithmetic is invalid.
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), ExecutionError> {
        self.apply_transaction_with_proposer(tx, PROTOCOL_FEE_SINK)
    }

    /// Apply one transaction and credit its fee to the proposer (or the protocol sink).
    ///
    /// # Errors
    /// Returns a typed error if signature, nonce, fee, solvency, or arithmetic is invalid.
    pub fn apply_transaction_with_proposer(
        &mut self,
        tx: &Transaction,
        proposer: PublicKeyBytes,
    ) -> Result<(), ExecutionError> {
        if tx.sender == tx.recipient {
            tracing::error!(account = ?tx.sender, "Percobaan transfer ke akun sendiri");
            return Err(ExecutionError::SelfTransferForbidden);
        }
        tx.verify_signature()?;
        if tx.fee == 0 {
            return Err(ExecutionError::FeeTooLow);
        }

        let sender_acc = self
            .accounts
            .get(&tx.sender)
            .copied()
            .ok_or(ExecutionError::AccountNotFound(tx.sender))?;
        if tx.nonce != sender_acc.nonce {
            return Err(ExecutionError::InvalidNonce {
                expected: sender_acc.nonce,
                got: tx.nonce,
            });
        }
        let total_outflow = tx
            .amount
            .checked_add(tx.fee)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        if sender_acc.balance < total_outflow {
            return Err(ExecutionError::InsufficientBalance {
                available: sender_acc.balance,
                required: total_outflow,
            });
        }

        let fee_recipient = if proposer == PROTOCOL_FEE_SINK {
            PROTOCOL_FEE_SINK
        } else {
            proposer
        };
        let recipient_acc = self
            .accounts
            .get(&tx.recipient)
            .copied()
            .unwrap_or(Account::new(0, 0));
        let fee_acc = self
            .accounts
            .get(&fee_recipient)
            .copied()
            .unwrap_or(Account::new(0, 0));
        let mut sender_balance = sender_acc
            .balance
            .checked_sub(total_outflow)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        let sender_nonce = sender_acc
            .nonce
            .checked_add(1)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        let mut recipient_balance = recipient_acc
            .balance
            .checked_add(tx.amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        let fee_balance = if fee_recipient != tx.sender && fee_recipient != tx.recipient {
            Some(
                fee_acc
                    .balance
                    .checked_add(tx.fee)
                    .ok_or(ExecutionError::ArithmeticOverflow)?,
            )
        } else {
            None
        };

        if fee_recipient == tx.sender {
            sender_balance = sender_balance
                .checked_add(tx.fee)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        } else if fee_recipient == tx.recipient {
            recipient_balance = recipient_balance
                .checked_add(tx.fee)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }

        // All validation and checked arithmetic has completed before mutating the state.
        self.accounts
            .insert(tx.sender, Account::new(sender_balance, sender_nonce));
        self.accounts.insert(
            tx.recipient,
            Account::new(recipient_balance, recipient_acc.nonce),
        );
        if let Some(balance) = fee_balance {
            self.accounts
                .insert(fee_recipient, Account::new(balance, fee_acc.nonce));
        }
        tracing::debug!(account = ?tx.sender, new_balance = sender_balance, fee = tx.fee, "Transaksi dan fee diterapkan");
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
