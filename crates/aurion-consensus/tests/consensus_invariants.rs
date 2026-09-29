#![forbid(unsafe_code)]

use aurion_consensus::{ConsensusError, RoundState, TimeoutVote, ValidatorSet, Vote, VoteType};
use aurion_criptografi::{Keypair, PublicKeyBytes};
use std::collections::BTreeSet;

fn key(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

fn validators(keys: &[Keypair]) -> ValidatorSet {
    ValidatorSet::new(keys.iter().map(Keypair::public_key_bytes).collect())
}

fn signed_vote(
    keypair: &Keypair,
    block_hash: [u8; 32],
    height: u64,
    round: u32,
    vote_type: VoteType,
) -> Vote {
    let mut vote = Vote::new(
        keypair.public_key_bytes(),
        block_hash,
        height,
        round,
        vote_type,
        [0; 64],
    );
    vote.signature = keypair.sign(&vote.digest());
    vote
}

fn signed_timeout(keypair: &Keypair, height: u64, round: u32) -> TimeoutVote {
    let mut vote = TimeoutVote::new(keypair.public_key_bytes(), height, round, [0; 64]);
    vote.signature = keypair.sign(&vote.digest());
    vote
}

#[test]
fn b0_verifies_membership_signature_and_keeps_rejected_votes_out() {
    let keys = vec![key(1), key(2), key(3), key(4)];
    let set = validators(&keys);
    let mut state = RoundState::new(1, 0, set.clone());
    let block = [11; 32];
    let valid = signed_vote(&keys[0], block, 1, 0, VoteType::Prevote);
    assert_eq!(state.add_vote(&valid), Ok(None));
    assert_eq!(state.vote_count(VoteType::Prevote, &block), 1);

    let mut bad_signature = signed_vote(&keys[1], block, 1, 0, VoteType::Prevote);
    bad_signature.signature[0] ^= 1;
    assert_eq!(
        state.add_vote(&bad_signature),
        Err(ConsensusError::InvalidVoteSignature)
    );

    let mut tampered_payload = signed_vote(&keys[1], block, 1, 0, VoteType::Prevote);
    tampered_payload.block_hash[0] ^= 1;
    assert_eq!(
        state.add_vote(&tampered_payload),
        Err(ConsensusError::InvalidVoteSignature)
    );

    let outsider = key(9);
    let unknown = signed_vote(&outsider, block, 1, 0, VoteType::Prevote);
    assert_eq!(
        state.add_vote(&unknown),
        Err(ConsensusError::UnknownValidator(
            outsider.public_key_bytes()
        ))
    );
    assert_eq!(state.vote_count(VoteType::Prevote, &block), 1);
}

#[test]
fn b1_forms_qc_only_at_two_f_plus_one_and_counts_each_validator_once() {
    let keys = vec![key(10), key(11), key(12), key(13)];
    let mut state = RoundState::new(5, 2, validators(&keys));
    let block = [42; 32];
    for keypair in &keys[..2] {
        assert_eq!(
            state.add_vote(&signed_vote(keypair, block, 5, 2, VoteType::Precommit)),
            Ok(None)
        );
    }
    let duplicate = signed_vote(&keys[0], block, 5, 2, VoteType::Precommit);
    assert_eq!(
        state.add_vote(&duplicate),
        Err(ConsensusError::DuplicateVote(keys[0].public_key_bytes()))
    );
    assert_eq!(state.vote_count(VoteType::Precommit, &block), 2);

    let qc = state.add_vote(&signed_vote(&keys[2], block, 5, 2, VoteType::Precommit));
    assert!(qc.is_ok());
    let qc = qc.ok().flatten();
    assert!(qc.is_some());
    if let Some(qc) = qc {
        assert_eq!(qc.height, 5);
        assert_eq!(qc.round, 2);
        assert_eq!(qc.block_hash, block);
        assert_eq!(qc.vote_type, VoteType::Precommit);
        assert_eq!(qc.signers.len(), 3);
        assert!(qc.signers.contains(&keys[0].public_key_bytes()));
    }
}

#[test]
fn b2_detects_equivocation_and_retains_both_signed_votes_as_evidence() {
    let keys = vec![key(20), key(21), key(22), key(23)];
    let mut state = RoundState::new(1, 0, validators(&keys));
    let first = signed_vote(&keys[0], [1; 32], 1, 0, VoteType::Prevote);
    let second = signed_vote(&keys[0], [2; 32], 1, 0, VoteType::Prevote);
    assert_eq!(state.add_vote(&first), Ok(None));
    assert_eq!(
        state.add_vote(&second),
        Err(ConsensusError::EquivocationDetected(
            keys[0].public_key_bytes()
        ))
    );
    assert_eq!(state.vote_count(VoteType::Prevote, &[1; 32]), 1);
    assert_eq!(state.vote_count(VoteType::Prevote, &[2; 32]), 0);
    let evidence = state.equivocation_evidence();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].first, first);
    assert_eq!(evidence[0].second, second);
    assert!(evidence[0].first.verify().is_ok());
    assert!(evidence[0].second.verify().is_ok());
}

#[test]
fn b3_stale_votes_are_rejected_and_round_advance_resets_accumulators() {
    let keys = vec![key(30), key(31), key(32), key(33)];
    let mut state = RoundState::new(8, 3, validators(&keys));
    let old_block = [3; 32];
    assert_eq!(
        state.add_vote(&signed_vote(&keys[0], old_block, 8, 3, VoteType::Prevote)),
        Ok(None)
    );
    assert_eq!(
        state.add_vote(&signed_vote(&keys[1], [4; 32], 8, 2, VoteType::Prevote)),
        Err(ConsensusError::StaleVote)
    );
    assert_eq!(
        state.add_vote(&signed_vote(&keys[1], [5; 32], 7, 3, VoteType::Prevote)),
        Err(ConsensusError::StaleVote)
    );
    assert_eq!(state.vote_count(VoteType::Prevote, &old_block), 1);

    assert!(state.advance_round().is_ok());
    assert_eq!(state.round, 4);
    assert_eq!(state.vote_count(VoteType::Prevote, &old_block), 0);
    assert_eq!(
        state.add_vote(&signed_vote(&keys[0], [6; 32], 8, 3, VoteType::Prevote)),
        Err(ConsensusError::StaleVote)
    );
    assert_eq!(
        state.add_vote(&signed_vote(&keys[0], [6; 32], 8, 4, VoteType::Prevote)),
        Ok(None)
    );

    let mut lock_state = RoundState::new(8, 3, validators(&keys));
    for keypair in &keys[..3] {
        let result = lock_state.add_vote(&signed_vote(keypair, [8; 32], 8, 3, VoteType::Prevote));
        assert!(result.is_ok());
    }
    assert!(lock_state.locked_qc().is_some());
    assert!(lock_state.advance_round().is_ok());
    assert_eq!(lock_state.vote_count(VoteType::Prevote, &[8; 32]), 0);
    assert!(lock_state.locked_qc().is_some());
}

#[test]
fn b4_timeout_quorum_advances_round_and_selects_leader_deterministically() {
    let keys = vec![key(40), key(41), key(42), key(43)];
    let set = validators(&keys);
    let expected_leader = set.leader_for(1, 1);
    assert!(expected_leader.is_ok());
    let mut state = RoundState::new(1, 0, set);
    for keypair in &keys[..2] {
        assert_eq!(
            state.add_timeout_vote(&signed_timeout(keypair, 1, 0)),
            Ok(None)
        );
    }
    let duplicate = signed_timeout(&keys[0], 1, 0);
    assert_eq!(
        state.add_timeout_vote(&duplicate),
        Err(ConsensusError::DuplicateTimeoutVote(
            keys[0].public_key_bytes()
        ))
    );
    let cert_result = state.add_timeout_vote(&signed_timeout(&keys[2], 1, 0));
    assert!(cert_result.is_ok());
    let cert = cert_result.ok().flatten();
    assert!(cert.is_some());
    if let (Some(cert), Ok(expected_leader)) = (cert, expected_leader) {
        assert_eq!(cert.height, 1);
        assert_eq!(cert.timed_out_round, 0);
        assert_eq!(cert.next_round, 1);
        assert_eq!(cert.signers.len(), 3);
        assert_eq!(cert.next_leader, expected_leader);
    }
    assert_eq!(state.round, 1);
    assert_eq!(
        state.add_timeout_vote(&signed_timeout(&keys[3], 1, 0)),
        Err(ConsensusError::StaleVote)
    );

    let mut max_round = RoundState::new(1, u32::MAX, validators(&keys));
    for keypair in &keys[..2] {
        assert_eq!(
            max_round.add_timeout_vote(&signed_timeout(keypair, 1, u32::MAX)),
            Ok(None)
        );
    }
    assert_eq!(
        max_round.add_timeout_vote(&signed_timeout(&keys[2], 1, u32::MAX)),
        Err(ConsensusError::ArithmeticOverflow)
    );
    assert_eq!(max_round.timeout_vote_count(), 2);
}

#[test]
fn b5_quorum_bounds_use_integer_arithmetic_and_leader_rotation_is_stable() {
    let cases = [
        (1_u8, 0_usize, 1_usize),
        (3, 0, 1),
        (4, 1, 3),
        (7, 2, 5),
        (10, 3, 7),
        (100, 33, 67),
    ];
    for (size, faults, quorum) in cases {
        let keys: Vec<Keypair> = (1..=size).map(key).collect();
        let set = validators(&keys);
        assert_eq!(set.max_faulty_nodes(), faults);
        assert_eq!(set.quorum_threshold(), Ok(quorum));
        let first = set.leader_for(17, 9);
        assert_eq!(first, set.leader_for(17, 9));
        assert!(first.is_ok());
    }
    let empty = ValidatorSet::new(Vec::new());
    assert_eq!(
        empty.leader_for(1, 0),
        Err(ConsensusError::EmptyValidatorSet)
    );

    let members: BTreeSet<PublicKeyBytes> = [
        key(70).public_key_bytes(),
        key(71).public_key_bytes(),
        key(72).public_key_bytes(),
    ]
    .into_iter()
    .collect();
    let set = ValidatorSet {
        members: members.clone(),
    };
    let expected_index = usize::try_from((6_u64 + 2_u64) % 3).unwrap_or_default();
    let expected = members.iter().nth(expected_index).copied();
    assert_eq!(set.leader_for(6, 2).ok(), expected);
}
