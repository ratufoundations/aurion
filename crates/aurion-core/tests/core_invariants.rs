#![forbid(unsafe_code)]

use aurion_core::{Account, Block, BlockHeader, ExecutionError, State, Transaction};
use aurion_criptografi::{Keypair, PublicKeyBytes};

fn key(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

fn signed_tx(sender: &Keypair, recipient: PublicKeyBytes, amount: u64, nonce: u64) -> Transaction {
    signed_tx_with_fee(sender, recipient, amount, nonce, 1)
}

fn signed_tx_with_fee(
    sender: &Keypair,
    recipient: PublicKeyBytes,
    amount: u64,
    nonce: u64,
    fee: u64,
) -> Transaction {
    let sender_pk = sender.public_key_bytes();
    let unsigned = Transaction::new(sender_pk, recipient, amount, nonce, fee, [0; 64]);
    Transaction::new(
        sender_pk,
        recipient,
        amount,
        nonce,
        fee,
        sender.sign(&unsigned.digest()),
    )
}

fn candidate_root(state: &State, txs: &[Transaction]) -> Result<[u8; 32], ExecutionError> {
    let mut candidate = state.clone();
    for tx in txs {
        candidate.apply_transaction(tx)?;
    }
    Ok(candidate.compute_state_root())
}

fn block_for(
    state: &State,
    height: u64,
    parent: &BlockHeader,
    txs: Vec<Transaction>,
) -> Result<Block, Box<dyn std::error::Error>> {
    let root = candidate_root(state, &txs)?;
    let tx_count = u32::try_from(txs.len())?;
    Ok(Block {
        header: BlockHeader {
            height,
            prev_hash: parent.hash(),
            state_root: root,
            tx_count,
            timestamp: height * 1_000,
            proposer: [0; 32],
        },
        transactions: txs,
    })
}

fn initial_state(keys: &[Keypair], balance: u64, nonce: u64) -> State {
    let mut state = State::new();
    for keypair in keys {
        state.insert_account(keypair.public_key_bytes(), Account::new(balance, nonce));
    }
    state
}

#[test]
fn st0_state_roots_are_deterministic_across_independent_replays_of_fifty_transactions(
) -> Result<(), Box<dyn std::error::Error>> {
    let senders: Vec<Keypair> = (1_u8..=5).map(key).collect();
    let recipient = [200_u8; 32];
    let mut state_a = initial_state(&senders, 10_000, 0);
    let mut state_b = State::new();
    for sender in senders.iter().rev() {
        state_b.insert_account(sender.public_key_bytes(), Account::new(10_000, 0));
    }
    assert_eq!(state_a.compute_state_root(), state_b.compute_state_root());

    let mut parent = BlockHeader {
        height: 0,
        prev_hash: [0; 32],
        state_root: state_a.compute_state_root(),
        tx_count: 0,
        timestamp: 0,
        proposer: [0; 32],
    };
    let mut replay_roots = Vec::new();
    for height in 1_u64..=10 {
        let nonce = height - 1;
        let txs: Vec<Transaction> = senders
            .iter()
            .map(|sender| signed_tx(sender, recipient, 1, nonce))
            .collect();
        let block = block_for(&state_a, height, &parent, txs)?;
        let expected_root = block.header.state_root;
        block.execute_with_parent(&mut state_a, &parent)?;
        block.execute_with_parent(&mut state_b, &parent)?;
        assert_eq!(state_a.compute_state_root(), expected_root);
        assert_eq!(state_b.compute_state_root(), expected_root);
        replay_roots.push(expected_root);
        parent = block.header.clone();
    }
    assert_eq!(replay_roots.len(), 10);
    assert_eq!(state_a.compute_state_root(), state_b.compute_state_root());
    Ok(())
}

#[test]
fn st1_invalid_transaction_rolls_back_every_prior_block_mutation() {
    let senders: Vec<Keypair> = (10_u8..14).map(key).collect();
    let recipient = key(30).public_key_bytes();
    let before = initial_state(&senders, 100, 0);
    let mut active_state = before.clone();
    let mut txs: Vec<Transaction> = senders
        .iter()
        .map(|sender| signed_tx(sender, recipient, 10, 0))
        .collect();
    txs.push(signed_tx(&senders[0], recipient, 1_000, 1));
    let block = Block {
        header: BlockHeader {
            height: 1,
            prev_hash: [0; 32],
            state_root: [0; 32],
            tx_count: 5,
            timestamp: 1_000,
            proposer: [0; 32],
        },
        transactions: txs,
    };
    assert!(matches!(
        block.execute(&mut active_state),
        Err(ExecutionError::InsufficientBalance { .. })
    ));
    assert_eq!(active_state, before);
}

#[test]
fn st2_transfers_conserve_quanta_and_overflow_or_underflow_is_rejected(
) -> Result<(), Box<dyn std::error::Error>> {
    let alice = key(40);
    let bob = key(41);
    let alice_pk = alice.public_key_bytes();
    let bob_pk = bob.public_key_bytes();
    let mut state = State::new();
    state.insert_account(alice_pk, Account::new(1_000, 0));
    state.insert_account(bob_pk, Account::new(70, 0));
    let total_before = state
        .accounts()
        .values()
        .map(|account| account.balance)
        .sum::<u64>();
    let transfer = signed_tx(&alice, bob_pk, 150, 0);
    state.apply_transaction(&transfer)?;
    assert_eq!(state.get_account(&alice_pk), Some(&Account::new(849, 1)));
    assert_eq!(state.get_account(&bob_pk), Some(&Account::new(220, 0)));
    let total_after = state
        .accounts()
        .values()
        .map(|account| account.balance)
        .sum::<u64>();
    assert_eq!(total_before, total_after);

    let before_underflow = state.clone();
    let overspend = signed_tx(&alice, bob_pk, 851, 1);
    assert!(matches!(
        state.apply_transaction(&overspend),
        Err(ExecutionError::InsufficientBalance { .. })
    ));
    assert_eq!(state, before_underflow);

    let overflow_sender = key(42);
    let overflow_sender_pk = overflow_sender.public_key_bytes();
    state.insert_account(overflow_sender_pk, Account::new(10, 0));
    state.insert_account(bob_pk, Account::new(u64::MAX, 0));
    let before_overflow = state.clone();
    let overflow = signed_tx(&overflow_sender, bob_pk, 1, 0);
    assert_eq!(
        state.apply_transaction(&overflow),
        Err(ExecutionError::ArithmeticOverflow)
    );
    assert_eq!(state, before_overflow);
    Ok(())
}

#[test]
fn st3_nonce_must_match_and_only_success_advances_it() {
    let alice = key(50);
    let bob = key(51);
    let alice_pk = alice.public_key_bytes();
    let bob_pk = bob.public_key_bytes();
    let mut state = State::new();
    state.insert_account(alice_pk, Account::new(500, 9));
    let current = signed_tx(&alice, bob_pk, 10, 9);
    assert!(state.apply_transaction(&current).is_ok());
    assert_eq!(state.get_account(&alice_pk), Some(&Account::new(489, 10)));

    let before = state.clone();
    let stale_tx = signed_tx(&alice, bob_pk, 1, 9);
    assert_eq!(
        state.apply_transaction(&stale_tx),
        Err(ExecutionError::InvalidNonce {
            expected: 10,
            got: 9
        })
    );
    let future = signed_tx(&alice, bob_pk, 1, 11);
    assert_eq!(
        state.apply_transaction(&future),
        Err(ExecutionError::InvalidNonce {
            expected: 10,
            got: 11
        })
    );
    assert_eq!(state, before);
}

#[test]
fn st4_fee_is_signed_debited_and_credited_to_proposer_or_protocol_sink() {
    use aurion_core::PROTOCOL_FEE_SINK;

    let alice = key(55);
    let bob = key(56);
    let proposer = key(57);
    let alice_pk = alice.public_key_bytes();
    let bob_pk = bob.public_key_bytes();
    let proposer_pk = proposer.public_key_bytes();
    let mut state = State::new();
    state.insert_account(alice_pk, Account::new(1_000, 0));
    state.insert_account(bob_pk, Account::new(50, 0));
    state.insert_account(proposer_pk, Account::new(25, 0));

    let tx = signed_tx_with_fee(&alice, bob_pk, 100, 0, 7);
    assert!(state
        .apply_transaction_with_proposer(&tx, proposer_pk)
        .is_ok());
    assert_eq!(state.get_account(&alice_pk), Some(&Account::new(893, 1)));
    assert_eq!(state.get_account(&bob_pk), Some(&Account::new(150, 0)));
    assert_eq!(state.get_account(&proposer_pk), Some(&Account::new(32, 0)));

    let mut tampered_fee_state = State::new();
    tampered_fee_state.insert_account(alice_pk, Account::new(1_000, 0));
    let mut tampered_fee = tx.clone();
    tampered_fee.fee = 8;
    assert_eq!(
        tampered_fee_state.apply_transaction(&tampered_fee),
        Err(ExecutionError::InvalidSignature)
    );
    assert_eq!(
        tampered_fee_state.get_account(&alice_pk),
        Some(&Account::new(1_000, 0))
    );

    let zero_fee = signed_tx_with_fee(&alice, bob_pk, 1, 0, 0);
    assert_eq!(
        tampered_fee_state.apply_transaction(&zero_fee),
        Err(ExecutionError::FeeTooLow)
    );
    assert_eq!(
        tampered_fee_state.get_account(&alice_pk),
        Some(&Account::new(1_000, 0))
    );

    let fallback_tx = signed_tx_with_fee(&alice, bob_pk, 1, 0, 2);
    assert!(tampered_fee_state.apply_transaction(&fallback_tx).is_ok());
    assert_eq!(
        tampered_fee_state.get_account(&PROTOCOL_FEE_SINK),
        Some(&Account::new(2, 0))
    );
}

#[test]
fn st5_parent_linkage_height_root_and_transaction_count_are_validated_atomically(
) -> Result<(), Box<dyn std::error::Error>> {
    let alice = key(60);
    let bob = key(61);
    let alice_pk = alice.public_key_bytes();
    let mut state = State::new();
    state.insert_account(alice_pk, Account::new(500, 0));
    let parent = BlockHeader {
        height: 4,
        prev_hash: [4; 32],
        state_root: state.compute_state_root(),
        tx_count: 0,
        timestamp: 4_000,
        proposer: [0; 32],
    };
    let tx = signed_tx(&alice, bob.public_key_bytes(), 10, 0);
    let valid = block_for(&state, 5, &parent, vec![tx.clone()])?;
    let mut executed = state.clone();
    valid.execute_with_parent(&mut executed, &parent)?;
    assert_eq!(executed.get_account(&alice_pk), Some(&Account::new(489, 1)));

    let mut wrong_parent = valid.clone();
    wrong_parent.header.prev_hash = [99; 32];
    let mut unchanged = state.clone();
    assert_eq!(
        wrong_parent.execute_with_parent(&mut unchanged, &parent),
        Err(ExecutionError::InvalidParentHash)
    );
    assert_eq!(unchanged, state);

    let mut skipped_height = valid.clone();
    skipped_height.header.height = 6;
    assert_eq!(
        skipped_height.execute_with_parent(&mut unchanged, &parent),
        Err(ExecutionError::NonSequentialHeight {
            expected: 5,
            got: 6
        })
    );
    assert_eq!(unchanged, state);
    let mut backward_height = valid.clone();
    backward_height.header.height = 3;
    assert_eq!(
        backward_height.execute_with_parent(&mut unchanged, &parent),
        Err(ExecutionError::NonSequentialHeight {
            expected: 5,
            got: 3
        })
    );
    assert_eq!(unchanged, state);

    let mut wrong_parent_state = parent.clone();
    wrong_parent_state.state_root = [77; 32];
    let mut child_of_wrong_state = valid.clone();
    child_of_wrong_state.header.prev_hash = wrong_parent_state.hash();
    assert_eq!(
        child_of_wrong_state.execute_with_parent(&mut unchanged, &wrong_parent_state),
        Err(ExecutionError::ParentStateRootMismatch)
    );
    assert_eq!(unchanged, state);

    let mut wrong_root = valid.clone();
    wrong_root.header.state_root = [88; 32];
    assert_eq!(
        wrong_root.execute_with_parent(&mut unchanged, &parent),
        Err(ExecutionError::StateRootMismatch)
    );
    assert_eq!(unchanged, state);

    let mut wrong_count = valid.clone();
    wrong_count.header.tx_count = 0;
    assert_eq!(
        wrong_count.execute_with_parent(&mut unchanged, &parent),
        Err(ExecutionError::InvalidTransactionCount)
    );
    let mut stale_timestamp = valid.clone();
    stale_timestamp.header.timestamp = parent.timestamp;
    assert_eq!(
        stale_timestamp.execute_with_parent(&mut unchanged, &parent),
        Err(ExecutionError::TimestampNotMonotonic)
    );
    assert_eq!(unchanged, state);
    Ok(())
}
