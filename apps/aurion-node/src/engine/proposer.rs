use aurion_consensus::{BlockProposal, ConsensusBlockHeader};
use aurion_core::{State, Transaction};
use aurion_criptografi::{Hasher, Keypair};
use aurion_mempool::Mempool;

pub const MAX_TX_PER_BLOCK: usize = 1_000;

pub fn harvest_and_propose(
    mempool: &Mempool,
    state: &State,
    keypair: &Keypair,
    chain_id: u32,
    height: u64,
    parent_hash: [u8; 32],
    timestamp: u64,
) -> Result<BlockProposal, Box<dyn std::error::Error>> {
    let candidates = mempool.select_transactions_for_block(state, MAX_TX_PER_BLOCK);
    if candidates.is_empty() {
        return Err("no transactions available".into());
    }

    let mut shadow = state.clone();
    let mut committed_txs = Vec::with_capacity(candidates.len());
    for tx in candidates {
        if shadow.apply_transaction(&tx).is_ok() {
            committed_txs.push(tx);
        }
    }
    if committed_txs.is_empty() {
        return Err("all transactions failed execution".into());
    }

    let state_root = shadow.compute_state_root();
    let tx_root = compute_tx_root(&committed_txs);

    let header = ConsensusBlockHeader {
        chain_id,
        height,
        timestamp,
        parent_hash,
        state_root,
        tx_root,
    };

    let mut preimage = Vec::with_capacity(156);
    preimage.extend_from_slice(&aurion_consensus::PROPOSAL_MAGIC);
    preimage.extend_from_slice(&header.encode());
    preimage.extend_from_slice(&(committed_txs.len() as u32).to_le_bytes());
    preimage.extend_from_slice(&tx_root);
    let signature = keypair.sign(&preimage);

    Ok(BlockProposal {
        header,
        transactions: committed_txs,
        proposer_signature: signature,
    })
}

fn compute_tx_root(txs: &[Transaction]) -> [u8; 32] {
    let mut buf = Vec::new();
    for tx in txs {
        buf.extend_from_slice(&aurion_ledger::Codec::encode_tx(tx));
    }
    Hasher::digest(&buf)
}
