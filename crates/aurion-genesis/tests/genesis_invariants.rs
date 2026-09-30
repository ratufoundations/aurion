#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use aurion_execution::{AccountKeeper, Keeper, ReplenishmentEngine, TransactionalCache};
use aurion_genesis::{
    build_synthetic_block_zero, compute_genesis_state_root, verify_genesis_state_root,
    AllocationRole, ConsensusGenesisParams, GenesisAllocation, GenesisConfig, GenesisError,
    GenesisValidator,
};
use std::collections::BTreeMap;

fn consensus_params() -> ConsensusGenesisParams {
    ConsensusGenesisParams {
        epoch_duration_blocks: 2_850,
        active_set_capacity: 21,
    }
}

fn conservative_allocations() -> Vec<GenesisAllocation> {
    vec![
        GenesisAllocation {
            account_id: [1u8; 32],
            role: AllocationRole::Creator,
            amount_quanta: 138_600_000_000_000_000,
        },
        GenesisAllocation {
            account_id: [2u8; 32],
            role: AllocationRole::Reservoir,
            amount_quanta: 521_400_000_000_000_000,
        },
    ]
}

fn validators() -> Vec<GenesisValidator> {
    vec![GenesisValidator {
        consensus_pubkey: [3u8; 32],
        stake_quanta: 0,
    }]
}

fn base_config() -> GenesisConfig {
    GenesisConfig {
        chain_id: 1001,
        genesis_time: 1_700_000_000,
        consensus: consensus_params(),
        allocations: conservative_allocations(),
        initial_validators: validators(),
    }
}

// ==============================================================================
// [GEN0] KEKETATAN SKEMA (Schema Strictness)
// ==============================================================================

#[test]
fn gen0_parses_canonical_config() {
    let creator_quanta = 138_600_000_000_000_000u128.to_string();
    let reservoir_quanta = 521_400_000_000_000_000u128.to_string();
    let vin = serde_json::json!({
        "chain_id": 1001,
        "genesis_time": 1_700_000_000,
        "consensus": { "epoch_duration_blocks": 2850, "active_set_capacity": 21 },
        "allocations": [
            { "account_id": vec![1u8; 32], "role": "creator",
              "amount_quanta": creator_quanta },
            { "account_id": vec![2u8; 32], "role": "reservoir",
              "amount_quanta": reservoir_quanta }
        ],
        "initial_validators": [
            { "consensus_pubkey": vec![3u8; 32], "stake_quanta": 0 }
        ]
    });
    let cfg: GenesisConfig =
        serde_json::from_value(vin).expect("konfigurasi kanonikal harus diterima");
    cfg.validate().expect("validasi konservasi lulus");
}

#[test]
fn gen0_rejects_unknown_field() {
    let mut value = serde_json::json!({
        "chain_id": 1001,
        "genesis_time": 1_700_000_000,
        "consensus": { "epoch_duration_blocks": 2850, "active_set_capacity": 21 },
        "allocations": [
            { "account_id": vec![1u8; 32], "role": "creator",
              "amount_quanta": "138600000000000000" }
        ],
        "initial_validators": [
            { "consensus_pubkey": vec![3u8; 32], "stake_quanta": 0 }
        ]
    });
    value["junk_field"] = serde_json::json!(42);
    let res: Result<GenesisConfig, _> = serde_json::from_value(value);
    assert!(res.is_err(), "field tak dikenal wajib ditolak");
}

#[test]
fn gen0_rejects_float_representation() {
    // Angka pecahan dibangun secara dinamis agar uji penolakan skema tetap
    // menguji masukan float tanpa pernah menulis literal float di sumber.
    let mut rendered = String::from("10");
    rendered.push('0');
    rendered.push('.');
    rendered.push('5');
    let float_number: serde_json::Value =
        serde_json::from_str(&rendered).expect("100.5 ter-render sebagai Number JSON");

    let mut vin = serde_json::json!({
        "chain_id": 1001,
        "genesis_time": 1_700_000_000,
        "consensus": { "epoch_duration_blocks": 2850, "active_set_capacity": 21 },
        "allocations": [
            { "account_id": vec![1u8; 32], "role": "creator",
              "amount_quanta": 1 }
        ],
        "initial_validators": [
            { "consensus_pubkey": vec![3u8; 32], "stake_quanta": 0 }
        ]
    });
    vin["allocations"][0]["amount_quanta"] = float_number;
    let res: Result<GenesisConfig, _> = serde_json::from_value(vin);
    assert!(res.is_err(), "representasi float wajib ditolak");
}

#[test]
fn gen0_rejects_negative_number() {
    let value = serde_json::json!({
        "chain_id": 1001,
        "genesis_time": 1_700_000_000,
        "consensus": { "epoch_duration_blocks": 2850, "active_set_capacity": 21 },
        "allocations": [
            { "account_id": vec![1u8; 32], "role": "creator",
              "amount_quanta": -100 }
        ],
        "initial_validators": [
            { "consensus_pubkey": vec![3u8; 32], "stake_quanta": 0 }
        ]
    });
    let res: Result<GenesisConfig, _> = serde_json::from_value(value);
    assert!(res.is_err(), "angka negatif wajib ditolak");
}

// ==============================================================================
// [GEN1] STATE ROOT DETERMINISTIK (Deterministic State Root)
// ==============================================================================

#[test]
fn gen1_shuffled_allocation_order_yields_identical_root() {
    let a = base_config();
    let mut b = base_config();
    b.allocations.reverse();

    let root_a = compute_genesis_state_root(&a).expect("root A valid");
    let root_b = compute_genesis_state_root(&b).expect("root B valid");
    assert_eq!(root_a, root_b, "state root wajib bebas urutan alokasi");
    assert_ne!(&a.allocations, &b.allocations);
}

// ==============================================================================
// [GEN2] BLOK SINTETIS #0 (Synthetic Block Zero)
// ==============================================================================

#[test]
fn gen2_synthetic_block_zero_shape_and_root() {
    let cfg = base_config();
    let state_root = compute_genesis_state_root(&cfg).expect("state root valid");
    let block = build_synthetic_block_zero(&cfg, state_root);

    assert_eq!(block.header.height, 0);
    assert_eq!(block.header.prev_hash, [0u8; 32]);
    assert_eq!(block.header.state_root, state_root);
    assert_eq!(block.header.tx_count, 0);
    assert_eq!(block.header.proposer, [0u8; 32]);
    assert!(block.transactions.is_empty());

    verify_genesis_state_root(&cfg, &block).expect("root blok 0 konsisten dengan kalkulasi");
}

// ==============================================================================
// [GEN3] KONSERVASI MONETER & SPLIT 21%
// ==============================================================================

#[test]
fn gen3_total_supply_is_conservative() {
    base_config()
        .validate_supply_conservation()
        .expect("total alokasi + stake = 660.000.000.000.000.000 Quanta");
}

#[test]
fn gen3_creator_receives_exactly_21_percent() {
    let cfg = base_config();
    let creator = cfg
        .allocations
        .iter()
        .find(|a| a.role == AllocationRole::Creator)
        .expect("alokasi creator ada");
    assert_eq!(creator.amount_quanta, 138_600_000_000_000_000);
    assert_eq!(
        creator.amount_quanta + 521_400_000_000_000_000,
        660_000_000_000_000_000
    );
}

#[test]
fn gen3_non_conservative_supply_is_rejected() {
    let mut cfg = base_config();
    cfg.allocations[1].amount_quanta = 521_400_000_000_000_000 - 1;
    match compute_genesis_state_root(&cfg) {
        Err(GenesisError::SupplyConservationMismatch { .. }) => {}
        other => panic!("Ekspektasi SupplyConservationMismatch, dapat: {other:?}"),
    }
}

// ==============================================================================
// [GEN4] VALIDASI HIMPUNAN VALIDATOR PERDANA
// ==============================================================================

#[test]
fn gen4_zero_validators_is_rejected() {
    let mut cfg = base_config();
    cfg.initial_validators.clear();
    match compute_genesis_state_root(&cfg) {
        Err(GenesisError::ZeroValidators) => {}
        other => panic!("Ekspektasi ZeroValidators, dapat: {other:?}"),
    }
}

#[test]
fn gen4_duplicate_validator_pubkey_is_rejected() {
    let mut cfg = base_config();
    cfg.initial_validators.push(GenesisValidator {
        consensus_pubkey: [3u8; 32],
        stake_quanta: 1_000,
    });
    match compute_genesis_state_root(&cfg) {
        Err(GenesisError::DuplicateValidatorPubkey(dup)) => assert_eq!(dup, [3u8; 32]),
        other => panic!("Ekspektasi DuplicateValidatorPubkey, dapat: {other:?}"),
    }
}

// ==============================================================================
// [GEN5] INVARIANT TRIGGER REPLENISHMENT
// ==============================================================================

#[test]
fn gen5_replenishes_zero_treasury_and_advances_cycle() {
    let treasury = [7u8; 32];
    let engine = ReplenishmentEngine::new().expect("engine replenishment dibuat");
    let accounts = AccountKeeper::new().expect("account keeper dibuat");
    let balance_key = accounts
        .store_key()
        .qualify(&accounts.balance_key(&treasury));

    let mut cache: TransactionalCache = TransactionalCache::new(BTreeMap::new(), 1_000_000);
    assert!(
        cache.get(&balance_key).expect("baca saldo").is_none(),
        "saldo treasury default nol"
    );

    let cycle = engine
        .check_and_replenish(&mut cache, &treasury)
        .expect("replenishment sukses");
    assert_eq!(cycle, Some(1), "cycle_index bergerak 0 -> 1");

    let refreshed = cache.get(&balance_key).expect("baca ulang saldo");
    let bal = refreshed.expect("saldo kini berisi nilai");
    assert_eq!(bal, 660_000_000_000_000_000u128.to_be_bytes().to_vec());
}

#[test]
fn gen5_no_replenishment_when_treasury_positive() {
    let treasury = [8u8; 32];
    let engine = ReplenishmentEngine::new().expect("engine dibuat");
    let accounts = AccountKeeper::new().expect("account keeper dibuat");
    let balance_key = accounts
        .store_key()
        .qualify(&accounts.balance_key(&treasury));

    let mut cache: TransactionalCache = TransactionalCache::new(BTreeMap::new(), 1_000_000);
    cache
        .set(balance_key, 100u128.to_be_bytes().to_vec())
        .expect("seeding saldo");
    assert_eq!(
        engine
            .check_and_replenish(&mut cache, &treasury)
            .expect("ok"),
        None,
        "saldo positif tidak memicu pencetakan"
    );
}
