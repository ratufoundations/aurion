use crate::config::{GenesisConfig, TOTAL_CYCLE_SUPPLY_QUANTA};
use crate::error::GenesisError;
use aurion_core::{Block, BlockHeader};

pub const GENESIS_STATE_ROOT_DOMAIN: &str = "AURION_GENESIS_STATE_ROOT_V1";
const NS_DIGEST_LEN: usize = 32;

/// Menghitung state root genesis secara deterministik dari pasangan
/// kunci-nilai yang diurutkan leksikografis biner murni.
///
/// Seluruh komitmen kunci menggunakan pemisahan domain BLAKE3:
/// `blake3("account")`, `blake3("validator")`, dan `blake3("system")`,
/// diikuti awalan label (`bal:`, `non:`, `rol:`, `sta:`, `stk:`, `param:`).
///
/// # Errors
/// Mengembalikan `ZeroValidators` atau `DuplicateValidatorPubkey` bila
/// himpunan validator perdana tidak sah, dan `SchemaViolation` bila konfigurasi
/// melanggar konservasi pasokan.
pub fn compute_genesis_state_root(config: &GenesisConfig) -> Result<[u8; 32], GenesisError> {
    config.validate()?;

    let account_ns = *blake3::hash(b"account").as_bytes();
    let validator_ns = *blake3::hash(b"validator").as_bytes();
    let system_ns = *blake3::hash(b"system").as_bytes();

    let mut kvs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();

    for alloc in &config.allocations {
        let mut bal_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 32);
        bal_key.extend_from_slice(&account_ns);
        bal_key.extend_from_slice(b"bal:");
        bal_key.extend_from_slice(&alloc.account_id);
        kvs.push((bal_key, alloc.amount_quanta.to_be_bytes().to_vec()));

        let mut nonce_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 32);
        nonce_key.extend_from_slice(&account_ns);
        nonce_key.extend_from_slice(b"non:");
        nonce_key.extend_from_slice(&alloc.account_id);
        kvs.push((nonce_key, [0u8; 8].to_vec()));

        let mut role_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 32);
        role_key.extend_from_slice(&account_ns);
        role_key.extend_from_slice(b"rol:");
        role_key.extend_from_slice(&alloc.account_id);
        kvs.push((role_key, [alloc.role.code()].to_vec()));
    }

    for val in &config.initial_validators {
        let mut status_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 32);
        status_key.extend_from_slice(&validator_ns);
        status_key.extend_from_slice(b"sta:");
        status_key.extend_from_slice(&val.consensus_pubkey);
        let status_byte = aurion_validator::status::ValidatorStatus::ActiveSet.code();
        kvs.push((status_key, [status_byte].to_vec()));

        let mut stake_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 32);
        stake_key.extend_from_slice(&validator_ns);
        stake_key.extend_from_slice(b"stk:");
        stake_key.extend_from_slice(&val.consensus_pubkey);
        kvs.push((stake_key, val.stake_quanta.to_be_bytes().to_vec()));
    }

    let mut cycle_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 16);
    cycle_key.extend_from_slice(&system_ns);
    cycle_key.extend_from_slice(b"param:cycle_index");
    kvs.push((cycle_key, 0u64.to_be_bytes().to_vec()));

    let mut minted_key = Vec::with_capacity(NS_DIGEST_LEN + 4 + 16);
    minted_key.extend_from_slice(&system_ns);
    minted_key.extend_from_slice(b"param:total_minted");
    kvs.push((minted_key, TOTAL_CYCLE_SUPPLY_QUANTA.to_be_bytes().to_vec()));

    kvs.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = blake3::Hasher::new_derive_key(GENESIS_STATE_ROOT_DOMAIN);
    for (key, val) in &kvs {
        let key_len = u32::try_from(key.len()).map_err(|_| {
            GenesisError::SchemaViolation("Panjang kunci melebihi jangkauan u32".into())
        })?;
        let val_len = u32::try_from(val.len()).map_err(|_| {
            GenesisError::SchemaViolation("Panjang nilai melebihi jangkauan u32".into())
        })?;
        hasher.update(&key_len.to_le_bytes());
        hasher.update(key);
        hasher.update(&val_len.to_le_bytes());
        hasher.update(val);
    }

    Ok(*hasher.finalize().as_bytes())
}

/// Membangun Blok Sintetis #0 (Genesis) sesuai Model A dari konfigurasi dan
/// state root hasil `compute_genesis_state_root`.
///
/// Header: `height = 0`, `prev_hash = [0u8; 32]`, `tx_count = 0`, proposer nol
/// mutlak, tanpa transaksi. Blok ini adalah acuan deterministik untuk semua
/// ketinggian berikutnya; `state_root`-nya wajib identik byte-per-byte pada
/// setiap inisialisasi dengan konfigurasi yang sama.
#[must_use]
pub fn build_synthetic_block_zero(config: &GenesisConfig, state_root: [u8; 32]) -> Block {
    Block {
        header: BlockHeader {
            height: 0,
            prev_hash: [0u8; 32],
            state_root,
            tx_count: 0,
            timestamp: config.genesis_time,
            proposer: [0u8; 32],
        },
        transactions: Vec::new(),
    }
}

/// Verifikasi bahwa `state_root` yang dihitung dari konfigurasi cocok dengan
/// state root yang tertanam pada Blok Sintetis #0.
///
/// # Errors
/// Mengembalikan `StateRootMismatch` bila kedua digest berbeda.
pub fn verify_genesis_state_root(
    config: &GenesisConfig,
    block: &Block,
) -> Result<(), GenesisError> {
    let calculated = compute_genesis_state_root(config)?;
    if calculated != block.header.state_root {
        return Err(GenesisError::StateRootMismatch {
            calculated,
            expected: block.header.state_root,
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::{
        AllocationRole, ConsensusGenesisParams, GenesisAllocation, GenesisValidator,
    };

    #[test]
    fn synthetic_block_zero_matches_model_a() {
        let config = GenesisConfig {
            chain_id: 1001,
            genesis_time: 1_700_000_000,
            consensus: ConsensusGenesisParams {
                epoch_duration_blocks: 2_850,
                active_set_capacity: 21,
            },
            allocations: vec![
                GenesisAllocation {
                    account_id: [1u8; 32],
                    role: AllocationRole::Creator,
                    amount_quanta: crate::config::CREATOR_ALLOCATION_QUANTA,
                },
                GenesisAllocation {
                    account_id: [2u8; 32],
                    role: AllocationRole::Reservoir,
                    amount_quanta: crate::config::RESERVOIR_ALLOCATION_QUANTA,
                },
            ],
            initial_validators: vec![GenesisValidator {
                consensus_pubkey: [3u8; 32],
                stake_quanta: 0,
            }],
        };
        let root = compute_genesis_state_root(&config).expect("state root valid");
        let block = build_synthetic_block_zero(&config, root);
        assert_eq!(block.header.height, 0);
        assert_eq!(block.header.prev_hash, [0u8; 32]);
        assert_eq!(block.header.tx_count, 0);
        assert_eq!(block.header.proposer, [0u8; 32]);
        assert!(block.transactions.is_empty());
        verify_genesis_state_root(&config, &block).expect("root konsisten");
    }
}
