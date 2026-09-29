use crate::error::MempoolError;
use aurion_core::{Account, Block, State, Transaction};
use aurion_criptografi::PublicKeyBytes;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

#[derive(Debug, Clone)]
pub struct MempoolConfig {
    pub max_total_transactions: usize,
    pub max_txs_per_account: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_total_transactions: 10_000,
            max_txs_per_account: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PooledTransaction {
    pub tx: Transaction,
    pub fee: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct CandidateKey {
    fee: u64,
    sender: PublicKeyBytes,
    nonce: u64,
}

impl Ord for CandidateKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fee
            .cmp(&other.fee)
            .then_with(|| other.nonce.cmp(&self.nonce))
    }
}

impl PartialOrd for CandidateKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct Mempool {
    config: MempoolConfig,
    by_sender: HashMap<PublicKeyBytes, BTreeMap<u64, PooledTransaction>>,
    total_tx_count: usize,
}

impl Mempool {
    #[must_use]
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            config,
            by_sender: HashMap::new(),
            total_tx_count: 0,
        }
    }
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.total_tx_count
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total_tx_count == 0
    }
    /// Memvalidasi dan menambahkan transaksi ke antrean sender.
    ///
    /// # Errors
    /// Mengembalikan error bila transaksi tidak valid, kapasitas habis, atau nonce bentrok.
    pub fn insert(
        &mut self,
        tx: Transaction,
        fee: u64,
        confirmed_state: &State,
    ) -> Result<(), MempoolError> {
        if tx.sender == tx.recipient {
            return Err(MempoolError::SelfTransfer);
        }
        if tx.amount == 0 {
            return Err(MempoolError::ZeroAmount);
        }
        if self.total_tx_count >= self.config.max_total_transactions {
            return Err(MempoolError::PoolCapacityReached {
                capacity: self.config.max_total_transactions,
            });
        }
        let sender_queue = self.by_sender.entry(tx.sender).or_default();
        if sender_queue.len() >= self.config.max_txs_per_account {
            return Err(MempoolError::AccountQueueLimitExceeded(
                tx.sender,
                self.config.max_txs_per_account,
            ));
        }
        if sender_queue.contains_key(&tx.nonce) {
            return Err(MempoolError::DuplicateNonce {
                sender: tx.sender,
                nonce: tx.nonce,
            });
        }
        tx.verify_signature()
            .map_err(|_| MempoolError::InvalidSignature)?;
        let confirmed_acc = confirmed_state
            .get_account(&tx.sender)
            .copied()
            .unwrap_or(Account::new(0, 0));
        if tx.nonce < confirmed_acc.nonce {
            tracing::warn!(tx_hash = ?tx.digest(), reason = "nonce usang", "Tx ditolak dari mempool");
            return Err(MempoolError::NonceTooLow {
                expected: confirmed_acc.nonce,
                got: tx.nonce,
            });
        }
        let total_pending_spend: u64 = sender_queue
            .values()
            .map(|p| p.tx.amount.saturating_add(p.fee))
            .sum();
        let total_required = total_pending_spend.saturating_add(tx.amount.saturating_add(fee));
        if confirmed_acc.balance < total_required {
            tracing::warn!(tx_hash = ?tx.digest(), reason = "saldo kurang", "Tx ditolak dari mempool");
            return Err(MempoolError::InsufficientBalance {
                available: confirmed_acc.balance,
                required: total_required,
            });
        }
        tracing::debug!(tx_hash = ?tx.digest(), fee, "Tx masuk antrean mempool");
        sender_queue.insert(tx.nonce, PooledTransaction { tx, fee });
        self.total_tx_count += 1;
        Ok(())
    }
    #[must_use]
    pub fn select_transactions_for_block(
        &self,
        confirmed_state: &State,
        max_txs: usize,
    ) -> Vec<Transaction> {
        if max_txs == 0 || self.total_tx_count == 0 {
            return Vec::new();
        }
        let mut selected = Vec::with_capacity(max_txs);
        let mut heap = BinaryHeap::new();
        let mut virtual_nonces: HashMap<PublicKeyBytes, u64> = HashMap::new();
        for (sender, queue) in &self.by_sender {
            let confirmed_nonce = confirmed_state.get_account(sender).map_or(0, |a| a.nonce);
            virtual_nonces.insert(*sender, confirmed_nonce);
            if let Some(candidate) = queue.get(&confirmed_nonce) {
                heap.push(CandidateKey {
                    fee: candidate.fee,
                    sender: *sender,
                    nonce: confirmed_nonce,
                });
            }
        }
        while let Some(top) = heap.pop() {
            if selected.len() >= max_txs {
                break;
            }
            if let Some(queue) = self.by_sender.get(&top.sender) {
                if let Some(pooled) = queue.get(&top.nonce) {
                    selected.push(pooled.tx.clone());
                    let next_nonce = top.nonce + 1;
                    virtual_nonces.insert(top.sender, next_nonce);
                    if let Some(next_tx) = queue.get(&next_nonce) {
                        heap.push(CandidateKey {
                            fee: next_tx.fee,
                            sender: top.sender,
                            nonce: next_nonce,
                        });
                    }
                }
            }
        }
        selected
    }
    pub fn prune_committed(&mut self, block: &Block, confirmed_state: &State) {
        for tx in &block.transactions {
            if let Some(queue) = self.by_sender.get_mut(&tx.sender) {
                if queue.remove(&tx.nonce).is_some() {
                    self.total_tx_count = self.total_tx_count.saturating_sub(1);
                }
            }
        }
        self.by_sender.retain(|sender, queue| {
            let confirmed_nonce = confirmed_state.get_account(sender).map_or(0, |a| a.nonce);
            let before_len = queue.len();
            queue.retain(|nonce, _| *nonce >= confirmed_nonce);
            let removed = before_len - queue.len();
            self.total_tx_count = self.total_tx_count.saturating_sub(removed);
            !queue.is_empty()
        });
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use aurion_criptografi::Keypair;
    fn make_tx(
        keypair: &Keypair,
        recipient: PublicKeyBytes,
        amount: u64,
        nonce: u64,
    ) -> Transaction {
        let sender = keypair.public_key_bytes();
        let payload = Transaction::payload_bytes(&sender, &recipient, amount, nonce);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"AURION_TX_CANONICAL_V1");
        hasher.update(&payload);
        let digest = *hasher.finalize().as_bytes();
        let sig = keypair.sign(&digest);
        Transaction::new(sender, recipient, amount, nonce, sig)
    }
    #[test]
    fn test_fee_prioritization_with_nonce_dependency() {
        let mut state = State::new();
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let charlie = Keypair::generate();
        let alice_pk = alice.public_key_bytes();
        let bob_pk = bob.public_key_bytes();
        let charlie_pk = charlie.public_key_bytes();
        state.insert_account(alice_pk, Account::new(1_000_000, 0));
        state.insert_account(bob_pk, Account::new(1_000_000, 0));
        let mut mempool = Mempool::new(MempoolConfig::default());
        let alice_tx0 = make_tx(&alice, charlie_pk, 1000, 0);
        let alice_tx1 = make_tx(&alice, charlie_pk, 1000, 1);
        let bob_tx0 = make_tx(&bob, charlie_pk, 2000, 0);
        mempool
            .insert(alice_tx0.clone(), 10, &state)
            .expect("test operation should succeed");
        mempool
            .insert(alice_tx1.clone(), 100, &state)
            .expect("test operation should succeed");
        mempool
            .insert(bob_tx0.clone(), 50, &state)
            .expect("test operation should succeed");
        assert_eq!(mempool.total_count(), 3);
        let block_txs = mempool.select_transactions_for_block(&state, 3);
        assert_eq!(block_txs.len(), 3);
        assert_eq!(block_txs[0].sender, bob_pk);
        assert_eq!(block_txs[0].nonce, 0);
        assert_eq!(block_txs[1].sender, alice_pk);
        assert_eq!(block_txs[1].nonce, 0);
        assert_eq!(block_txs[2].sender, alice_pk);
        assert_eq!(block_txs[2].nonce, 1);
    }
    #[test]
    fn test_reject_low_nonce_and_insufficient_balance() {
        let mut state = State::new();
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let alice_pk = alice.public_key_bytes();
        let bob_pk = bob.public_key_bytes();
        state.insert_account(alice_pk, Account::new(500, 2));
        let mut mempool = Mempool::new(MempoolConfig::default());
        let tx_low_nonce = make_tx(&alice, bob_pk, 100, 1);
        let err = mempool.insert(tx_low_nonce, 10, &state).unwrap_err();
        assert_eq!(
            err,
            MempoolError::NonceTooLow {
                expected: 2,
                got: 1
            }
        );
        let tx_overspend = make_tx(&alice, bob_pk, 500, 2);
        let err = mempool.insert(tx_overspend, 10, &state).unwrap_err();
        assert_eq!(
            err,
            MempoolError::InsufficientBalance {
                available: 500,
                required: 510
            }
        );
    }
}
