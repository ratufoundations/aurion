use aurion_consensus::{BlockProposal, PrecommitVote};
use aurion_core::State;
use aurion_criptografi::Keypair;

pub fn validate_and_vote(
    proposal: &BlockProposal,
    state: &State,
    keypair: &Keypair,
    proposer_pubkey: &[u8; 32],
) -> Result<PrecommitVote, Box<dyn std::error::Error>> {
    if !proposal.verify_signature(proposer_pubkey)? {
        return Err("invalid proposer signature".into());
    }

    let mut shadow = state.clone();
    for tx in &proposal.transactions {
        shadow.apply_transaction(tx)?;
    }

    let calculated_root = shadow.compute_state_root();
    if calculated_root != proposal.header.state_root {
        return Err("state root mismatch".into());
    }

    let block_hash = proposal.header.hash();
    let voter_pubkey = keypair.public_key_bytes();
    let mut preimage = Vec::with_capacity(76);
    preimage.extend_from_slice(&0u32.to_le_bytes());
    preimage.extend_from_slice(&proposal.header.height.to_le_bytes());
    preimage.extend_from_slice(&block_hash);
    preimage.extend_from_slice(&voter_pubkey);
    let signature = keypair.sign(&preimage);

    Ok(PrecommitVote::new(
        0,
        proposal.header.height,
        block_hash,
        voter_pubkey,
        signature,
    ))
}
