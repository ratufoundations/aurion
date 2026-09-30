#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use aurion_consensus::{
    calculate_quorum, BlockProposal, ConsensusBlockHeader, PrecommitVote, QuorumCertificate,
    ValidatorSet, HEADER_SIZE, MIN_PROPOSAL_SIZE, PRECOMMIT_VOTE_SIZE, PROPOSAL_MAGIC,
};
use aurion_criptografi::{Keypair, PublicKeyBytes};
use std::error::Error;

fn keypair(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

fn header(height: u64) -> ConsensusBlockHeader {
    ConsensusBlockHeader {
        chain_id: 1001,
        height,
        timestamp: 1_700_000_000,
        parent_hash: [0u8; 32],
        state_root: [1u8; 32],
        tx_root: [2u8; 32],
    }
}

fn sign_proposal(
    header: &ConsensusBlockHeader,
    tx_count: u32,
    tx_root: [u8; 32],
    keypair: &Keypair,
) -> [u8; 64] {
    let mut preimage = Vec::with_capacity(156);
    preimage.extend_from_slice(&PROPOSAL_MAGIC);
    preimage.extend_from_slice(&header.encode());
    preimage.extend_from_slice(&tx_count.to_le_bytes());
    preimage.extend_from_slice(&tx_root);
    keypair.sign(&preimage)
}

fn sign_vote(round: u32, height: u64, block_hash: [u8; 32], keypair: &Keypair) -> [u8; 64] {
    let mut preimage = Vec::with_capacity(76);
    preimage.extend_from_slice(&round.to_le_bytes());
    preimage.extend_from_slice(&height.to_le_bytes());
    preimage.extend_from_slice(&block_hash);
    preimage.extend_from_slice(&keypair.public_key_bytes());
    keypair.sign(&preimage)
}

#[test]
fn test_cm0_wire_codec_roundtrip_empty_proposal() -> Result<(), Box<dyn Error>> {
    let kp = keypair(1);
    let h = header(0);
    let sig = sign_proposal(&h, 0, h.tx_root, &kp);
    let proposal = BlockProposal {
        header: h.clone(),
        transactions: vec![],
        proposer_signature: sig,
    };

    let encoded = proposal.encode();
    assert_eq!(encoded.len(), 188);
    assert_eq!(188, 4 + 116 + 4 + 64);

    let decoded = BlockProposal::decode(&encoded)?;
    assert_eq!(decoded.header, h);
    assert_eq!(decoded.transactions.len(), 0);
    assert_eq!(decoded.proposer_signature, sig);
    Ok(())
}

#[test]
fn test_cm0_rejects_invalid_magic() {
    let kp = keypair(1);
    let h = header(0);
    let sig = sign_proposal(&h, 0, h.tx_root, &kp);
    let proposal = BlockProposal {
        header: h,
        transactions: vec![],
        proposer_signature: sig,
    };
    let mut encoded = proposal.encode();
    encoded[0] = 0xFF;
    assert!(BlockProposal::decode(&encoded).is_err());
}

#[test]
fn test_cm0_rejects_short_buffer() {
    let buf = vec![0u8; 100];
    assert!(BlockProposal::decode(&buf).is_err());
}

#[test]
fn test_cm1_header_hash_is_deterministic() {
    let h1 = header(5);
    let h2 = header(5);
    assert_eq!(h1.hash(), h2.hash());
    assert_eq!(h1.encode().len(), HEADER_SIZE);
}

#[test]
fn test_cm2_precommit_vote_roundtrip_and_verify() -> Result<(), Box<dyn Error>> {
    let kp = keypair(2);
    let h = header(10);
    let block_hash = h.hash();
    let sig = sign_vote(0, 10, block_hash, &kp);
    let vote = PrecommitVote::new(0, 10, block_hash, kp.public_key_bytes(), sig);

    let encoded = vote.encode();
    assert_eq!(encoded.len(), PRECOMMIT_VOTE_SIZE);
    assert_eq!(PRECOMMIT_VOTE_SIZE, 140);

    let decoded = PrecommitVote::decode(&encoded)?;
    assert_eq!(decoded.round, 0);
    assert_eq!(decoded.height, 10);
    assert_eq!(decoded.block_hash, block_hash);
    assert!(decoded.verify()?);
    Ok(())
}

#[test]
fn test_cm2_vote_rejects_forged_signature() -> Result<(), Box<dyn Error>> {
    let kp = keypair(2);
    let h = header(10);
    let block_hash = h.hash();
    let mut sig = sign_vote(0, 10, block_hash, &kp);
    sig[0] ^= 0xFF;
    let vote = PrecommitVote::new(0, 10, block_hash, kp.public_key_bytes(), sig);
    assert!(!vote.verify()?);
    Ok(())
}

#[test]
fn test_cm3_calculate_quorum() {
    assert_eq!(calculate_quorum(1).ok(), Some(1));
    assert_eq!(calculate_quorum(2).ok(), Some(2));
    assert_eq!(calculate_quorum(3).ok(), Some(3));
    assert_eq!(calculate_quorum(4).ok(), Some(3));
    assert!(calculate_quorum(0).is_err());
}

#[test]
fn test_cm3_solo_bootstrap_n1_quorum() -> Result<(), Box<dyn Error>> {
    let kp = keypair(1);
    let vs = ValidatorSet::new(vec![kp.public_key_bytes()]);
    let h = header(0);
    let block_hash = h.hash();
    let sig = sign_vote(0, 0, block_hash, &kp);
    let qc = QuorumCertificate::new(block_hash, 0, 0, vec![(kp.public_key_bytes(), sig)]);
    assert!(qc.verify(&vs)?);
    Ok(())
}

#[test]
fn test_cm3_multi_node_n4_requires_3_votes() -> Result<(), Box<dyn Error>> {
    let kps: Vec<Keypair> = (1..=4).map(keypair).collect();
    let pubkeys: Vec<PublicKeyBytes> = kps.iter().map(Keypair::public_key_bytes).collect();
    let vs = ValidatorSet::new(pubkeys);
    let h = header(1);
    let block_hash = h.hash();

    let votes: Vec<(PublicKeyBytes, [u8; 64])> = kps
        .iter()
        .map(|k| (k.public_key_bytes(), sign_vote(0, 1, block_hash, k)))
        .collect();

    let qc2 = QuorumCertificate::new(block_hash, 1, 0, votes[..2].to_vec());
    assert!(qc2.verify(&vs).is_err());

    let qc3 = QuorumCertificate::new(block_hash, 1, 0, votes[..3].to_vec());
    assert!(qc3.verify(&vs)?);
    Ok(())
}

#[test]
fn test_cm3_stall_with_only_2_of_4() {
    let kps: Vec<Keypair> = (1..=4).map(keypair).collect();
    let pubkeys: Vec<PublicKeyBytes> = kps.iter().map(Keypair::public_key_bytes).collect();
    let vs = ValidatorSet::new(pubkeys);
    let h = header(1);
    let block_hash = h.hash();
    let votes: Vec<(PublicKeyBytes, [u8; 64])> = kps
        .iter()
        .take(2)
        .map(|k| (k.public_key_bytes(), sign_vote(0, 1, block_hash, k)))
        .collect();
    let qc = QuorumCertificate::new(block_hash, 1, 0, votes);
    assert!(qc.verify(&vs).is_err());
}
