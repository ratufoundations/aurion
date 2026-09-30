use aurion_consensus::{calculate_quorum, QuorumCertificate};
use aurion_criptografi::{PublicKeyBytes, SignatureBytes};

#[derive(Debug)]
pub struct CommitResult {
    pub block_hash: [u8; 32],
    pub height: u64,
    pub tx_count: u32,
}

#[allow(clippy::too_many_arguments)]
pub fn commit_with_quorum(
    _store: &aurion_ledger::LedgerStore,
    _mempool: &aurion_mempool::Mempool,
    _state: &mut aurion_core::State,
    votes: &[(PublicKeyBytes, SignatureBytes)],
    validator_count: usize,
    block_hash: [u8; 32],
    height: u64,
    round: u32,
) -> Result<CommitResult, Box<dyn std::error::Error>> {
    let qc = QuorumCertificate::new(block_hash, height, round, votes.to_vec());
    let validator_set = aurion_consensus::ValidatorSet::new(vec![]);
    let _ = calculate_quorum(validator_count)?;
    qc.verify(&validator_set)?;

    Ok(CommitResult {
        block_hash,
        height,
        tx_count: votes.len() as u32,
    })
}
