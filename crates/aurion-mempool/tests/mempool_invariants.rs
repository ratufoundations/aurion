#![forbid(unsafe_code)]

use aurion_core::{Account, Block, BlockHeader, State, Transaction};
use aurion_criptografi::{Keypair, PublicKeyBytes};
use aurion_mempool::{Mempool, MempoolConfig, MempoolError};

fn key(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

fn signed_tx(
    keypair: &Keypair,
    recipient: PublicKeyBytes,
    amount: u128,
    nonce: u64,
    fee: u128,
) -> Transaction {
    let sender = keypair.public_key_bytes();
    let unsigned = Transaction::new(sender, recipient, amount, nonce, fee, [0_u8; 64]);
    let signature = keypair.sign(&unsigned.digest());
    Transaction::new(sender, recipient, amount, nonce, fee, signature)
}

fn funded_state(keys: &[&Keypair], balance: u128, nonce: u64) -> State {
    let mut state = State::new();
    for keypair in keys {
        state.insert_account(keypair.public_key_bytes(), Account::new(balance, nonce));
    }
    state
}

fn config(capacity: usize, per_account: usize, minimum_fee: u128) -> MempoolConfig {
    MempoolConfig {
        max_total_transactions: capacity,
        max_txs_per_account: per_account,
        minimum_fee,
    }
}

#[test]
fn m0_accepts_valid_signature_and_rejects_tampering_without_insertion() {
    let alice = key(1);
    let bob = key(2);
    let state = funded_state(&[&alice], 1_000, 0);
    let mut pool = Mempool::new(MempoolConfig::default());
    let valid = signed_tx(&alice, bob.public_key_bytes(), 10, 0, 1);
    assert_eq!(pool.insert(valid.clone(), &state), Ok(()));
    assert_eq!(pool.total_count(), 1);

    let mut tampered_payload = valid.clone();
    tampered_payload.amount += 1;
    assert_eq!(
        pool.insert(tampered_payload, &state),
        Err(MempoolError::InvalidSignature)
    );

    let mut tampered_signature = valid.clone();
    tampered_signature.signature[0] ^= 1;
    assert_eq!(
        pool.insert(tampered_signature, &state),
        Err(MempoolError::InvalidSignature)
    );

    let wrong_sender = key(3);
    let mut mismatched_sender = valid;
    mismatched_sender.sender = wrong_sender.public_key_bytes();
    assert_eq!(
        pool.insert(mismatched_sender, &state),
        Err(MempoolError::InvalidSignature)
    );
    assert_eq!(pool.total_count(), 1);
}

#[test]
fn m1_enforces_next_nonce_replay_and_duplicate_rules() {
    let alice = key(4);
    let bob = key(5);
    let state = funded_state(&[&alice], 10_000, 7);
    let mut pool = Mempool::new(MempoolConfig::default());
    let old_nonce_tx = signed_tx(&alice, bob.public_key_bytes(), 10, 6, 1);
    assert_eq!(
        pool.insert(old_nonce_tx, &state),
        Err(MempoolError::NonceTooLow {
            expected: 7,
            got: 6
        })
    );

    let current = signed_tx(&alice, bob.public_key_bytes(), 10, 7, 1);
    assert_eq!(pool.insert(current.clone(), &state), Ok(()));
    assert_eq!(
        pool.insert(current.clone(), &state),
        Err(MempoolError::DuplicateTransaction)
    );
    let conflicting = signed_tx(&alice, bob.public_key_bytes(), 11, 7, 1);
    assert_eq!(
        pool.insert(conflicting, &state),
        Err(MempoolError::DuplicateNonce {
            sender: alice.public_key_bytes(),
            nonce: 7
        })
    );
    let next = signed_tx(&alice, bob.public_key_bytes(), 10, 8, 1);
    assert_eq!(pool.insert(next, &state), Ok(()));
    assert_eq!(pool.total_count(), 2);
}

#[test]
fn m2_checks_quanta_solvency_overflow_and_minimum_fee() {
    let alice = key(6);
    let bob = key(7);
    let state = funded_state(&[&alice], 100, 0);
    let mut pool = Mempool::new(config(10, 10, 5));
    let low_fee = signed_tx(&alice, bob.public_key_bytes(), 1, 0, 4);
    assert_eq!(
        pool.insert(low_fee, &state),
        Err(MempoolError::FeeTooLow { fee: 4, minimum: 5 })
    );

    let enough = signed_tx(&alice, bob.public_key_bytes(), 95, 0, 5);
    assert_eq!(pool.insert(enough, &state), Ok(()));
    let overspend = signed_tx(&alice, bob.public_key_bytes(), 1, 1, 5);
    assert_eq!(
        pool.insert(overspend, &state),
        Err(MempoolError::InsufficientBalance {
            available: 100,
            required: 106
        })
    );

    let rich_state = funded_state(&[&alice], u128::MAX, 0);
    let mut overflow_pool = Mempool::new(MempoolConfig::default());
    let overflow = signed_tx(&alice, bob.public_key_bytes(), u128::MAX, 0, 1);
    assert_eq!(
        overflow_pool.insert(overflow, &rich_state),
        Err(MempoolError::ArithmeticOverflow)
    );
}

#[test]
fn m3_selects_one_hundred_transactions_by_deterministic_fee_priority() {
    let keys: Vec<Keypair> = (10_u8..110).map(key).collect();
    let refs: Vec<&Keypair> = keys.iter().collect();
    let state = funded_state(&refs, 1_000, 0);
    let recipient = key(240).public_key_bytes();
    let mut pool = Mempool::new(MempoolConfig::default());
    for (index, keypair) in keys.iter().enumerate() {
        let fee = u128::try_from((index * 37) % 101 + 1).unwrap_or_default();
        let tx = signed_tx(keypair, recipient, 1, 0, fee);
        assert_eq!(pool.insert(tx, &state), Ok(()));
    }
    let selected = pool.select_transactions_for_block(&state, 100);
    assert_eq!(selected.len(), 100);
    let fees: Vec<u64> = selected
        .iter()
        .map(|tx| {
            keys.iter()
                .position(|candidate| candidate.public_key_bytes() == tx.sender)
                .map_or(0, |index| {
                    u64::try_from((index * 37) % 101 + 1).unwrap_or_default()
                })
        })
        .collect();
    assert!(fees.windows(2).all(|pair| pair[0] >= pair[1]));
    let second_run = pool.select_transactions_for_block(&state, 100);
    assert_eq!(selected, second_run);
}

#[test]
fn m4_pool_rejects_low_fee_and_evicts_the_lowest_fee_for_higher_fee() {
    let alice = key(120);
    let bob = key(121);
    let carol = key(122);
    let state = funded_state(&[&alice, &bob, &carol], 1_000, 0);
    let recipient = key(123).public_key_bytes();
    let mut pool = Mempool::new(config(2, 4, 1));
    let low = signed_tx(&alice, recipient, 10, 0, 5);
    let middle = signed_tx(&bob, recipient, 10, 0, 10);
    assert_eq!(pool.insert(low.clone(), &state), Ok(()));
    assert_eq!(pool.insert(middle.clone(), &state), Ok(()));
    let rejected = signed_tx(&carol, recipient, 10, 0, 4);
    assert_eq!(pool.insert(rejected, &state), Err(MempoolError::PoolFull));
    let high = signed_tx(&carol, recipient, 10, 0, 20);
    assert_eq!(pool.insert(high.clone(), &state), Ok(()));
    assert_eq!(pool.total_count(), 2);
    let selected = pool.select_transactions_for_block(&state, 10);
    assert_eq!(selected.len(), 2);
    assert!(selected.contains(&middle));
    assert!(selected.contains(&high));
    assert!(!selected.contains(&low));
}

#[test]
fn m5_prunes_committed_transactions_and_keeps_uncommitted_nonce_successors(
) -> Result<(), Box<dyn std::error::Error>> {
    let alice = key(130);
    let bob = key(131);
    let carol = key(132);
    let alice_pk = alice.public_key_bytes();
    let bob_pk = bob.public_key_bytes();
    let carol_pk = carol.public_key_bytes();
    let mut state = funded_state(&[&alice, &bob], 1_000, 0);
    let mut pool = Mempool::new(MempoolConfig::default());
    let alice_first = signed_tx(&alice, carol_pk, 10, 0, 1);
    let alice_second = signed_tx(&alice, carol_pk, 10, 1, 1);
    let bob_first = signed_tx(&bob, carol_pk, 10, 0, 1);
    let bob_second = signed_tx(&bob, carol_pk, 10, 1, 1);
    for tx in [&alice_first, &alice_second, &bob_first, &bob_second] {
        assert_eq!(pool.insert(tx.clone(), &state), Ok(()));
    }
    state.apply_transaction(&alice_first)?;
    state.apply_transaction(&bob_first)?;
    let block = Block {
        header: BlockHeader {
            height: 1,
            prev_hash: [0; 32],
            state_root: state.compute_state_root(),
            tx_count: 2,
            timestamp: 1_000,
            proposer: [0; 32],
        },
        transactions: vec![alice_first.clone(), bob_first.clone()],
    };
    pool.prune_committed(&block, &state);
    assert_eq!(pool.total_count(), 2);
    let remaining = pool.select_transactions_for_block(&state, 10);
    assert_eq!(remaining.len(), 2);
    assert!(remaining.contains(&alice_second));
    assert!(remaining.contains(&bob_second));
    assert!(!remaining.contains(&alice_first));
    assert!(!remaining.contains(&bob_first));
    assert_ne!(alice_pk, bob_pk);
    Ok(())
}
