#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Matriks invarian orkestrasi O0-O5 untuk `aurion-node`.
//!
//! Setiap test memverifikasi satu kanal atau saturasi. Tidak ada mock: seluruh
//! test menjalankan komponen Aurion sungguhan di atas `tokio` runtime.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use aurion_core::Transaction;
use aurion_criptografi::{Keypair, PublicKeyBytes, SignatureBytes};
use aurion_guard::{BlacklistVerdict, RaidEvidence, ViolationType};
use aurion_ledger::LedgerStore;
use aurion_network::{AurionWireCodec, NetworkMessage};

use aurion_consensus::{Vote, VoteType};
use aurion_node::node::{
    decode_gossip, default_guards, encode_gossip, sign_vote, ChainEvent, ChainNode, NodeConfig,
    NodeHandle,
};

const TIMEOUT: Duration = Duration::from_secs(10);

/// Basis data redb adalah satu *file*, bukan direktori.
fn test_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aurion-node-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("ledger.redb")
}

fn config_with(dir: &Path, treasury: PublicKeyBytes) -> NodeConfig {
    NodeConfig {
        data_dir: dir.to_path_buf(),
        treasury_account: treasury,
        ..NodeConfig::default()
    }
}

/// Tanda tangani payload dengan kunci pengirim.
fn signed_transfer(
    sender: &Keypair,
    recipient: PublicKeyBytes,
    amount: u64,
    nonce: u64,
    fee: u64,
) -> Transaction {
    let mut tx = Transaction::new(
        sender.public_key_bytes(),
        recipient,
        amount,
        nonce,
        fee,
        [0u8; 64],
    );
    let signature = sender.sign(&tx.digest());
    tx.signature = signature;
    tx
}

async fn next_event(runtime: &mut aurion_node::node::NodeRuntime) -> ChainEvent {
    runtime
        .next_event_timeout(TIMEOUT)
        .await
        .expect("actor harus memublikasikan kejadian dalam batas waktu")
}

/// Buang kejadian sampai predicate terpenuhi.
async fn wait_for(
    runtime: &mut aurion_node::node::NodeRuntime,
    mut predicate: impl FnMut(&ChainEvent) -> bool,
) -> ChainEvent {
    for _ in 0..64 {
        let event = next_event(runtime).await;
        if predicate(&event) {
            return event;
        }
    }
    panic!("kejadian yang diharapkan tidak pernah tiba");
}

/// Konsumsi kejadian hingga satu `VotesObserved` mencapai jumlah suara sasaran
/// pada tinggi tertentu, lalu coret penghitung tertinggi yang teramati.
async fn await_votes(runtime: &mut aurion_node::node::NodeRuntime, height: u64, target: usize) {
    let mut peak = 0;
    for _ in 0..96 {
        let event = next_event(runtime).await;
        if let ChainEvent::VotesObserved {
            height: seen_height,
            votes,
        } = event
        {
            assert_eq!(seen_height, height, "tinggi suara tidak cocok");
            peak = peak.max(votes);
            if peak >= target {
                return;
            }
        }
    }
    panic!("kuorum {target} suara pada tinggi {height} tidak pernah tercapai, puncak {peak}");
}

// ---------------------------------------------------------------- O0

/// O0: bootstrapping genesis menulis blok 0, dan pembukaan ulang tidak
/// menggandakan genesis.
#[tokio::test]
async fn o0_genesis_is_committed_once_and_reopen_is_idempotent() {
    let dir = test_dir("o0");
    let kp = Keypair::generate();
    let config = config_with(&dir, kp.public_key_bytes());

    let node = ChainNode::bootstrap(config.clone()).expect("bootstrap genesis");
    assert_eq!(node.last_height(), 0, "blok 0 harus ada setelah bootstrap");
    drop(node);
    let genesis_root = {
        let ledger = LedgerStore::open(&dir).expect("buka ledger");
        let block = ledger
            .get_block_by_height(0)
            .expect("kueri blok 0")
            .expect("blok 0 harus tersimpan");
        block.header.state_root
    };

    let reopened = ChainNode::bootstrap(config).expect("bootstrap kedua");
    assert_eq!(
        reopened.last_height(),
        0,
        "pembukaan ulang tidak boleh menambah blok"
    );
    drop(reopened);

    let ledger = LedgerStore::open(&dir).expect("buka ledger ulang");
    assert_eq!(
        ledger.get_block_by_height(1).expect("kueri blok 1"),
        None,
        "tidak boleh ada blok 1 dari bootstrap"
    );
    let again = ledger
        .get_block_by_height(0)
        .expect("kueri blok 0")
        .expect("blok 0 masih ada");
    assert_eq!(again.header.state_root, genesis_root, "state root stabil");
}

/// O0: konfigurasi sama menghasilkan `state_root` genesis yang deterministik
/// pada direktori berbeda.
#[tokio::test]
async fn o0_genesis_state_root_is_deterministic() {
    let kp = Keypair::generate();
    let dir_a = test_dir("o0-a");
    let dir_b = test_dir("o0-b");

    let first =
        ChainNode::bootstrap(config_with(&dir_a, kp.public_key_bytes())).expect("bootstrap a");
    let second =
        ChainNode::bootstrap(config_with(&dir_b, kp.public_key_bytes())).expect("bootstrap b");
    assert_eq!(first.last_height(), 0);
    assert_eq!(second.last_height(), 0);
    // Lepaskan handle redb sebelumFuel membuka basis data yang sama.
    drop(first);
    drop(second);

    let root_of = |dir: &Path| {
        LedgerStore::open(dir)
            .expect("ledger")
            .get_block_by_height(0)
            .expect("q")
            .expect("blok 0")
            .header
            .state_root
    };
    assert_eq!(
        root_of(&dir_a),
        root_of(&dir_b),
        "genesis harus deterministik"
    );
}

// ---------------------------------------------------------------- O1

/// O1: kanal ingress berbatas menghasilkan backpressure, bukan penumpukan
/// memori. Verifikasi signature Ed25519 pada actor jauh lebih lambat daripada
/// loop pengirim, sehingga kanal pasti penuh.
#[tokio::test]
async fn o1_ingress_channel_applies_backpressure() {
    let dir = test_dir("o1");
    let kp = Keypair::generate();
    let mut config = config_with(&dir, kp.public_key_bytes());
    config.ingress_capacity = 4;
    config.initial_guards = default_guards();

    let node = ChainNode::bootstrap(config).expect("bootstrap");
    let runtime = NodeHandle::spawn(node);
    let handle = runtime.handle.clone();
    let recipient = Keypair::generate().public_key_bytes();

    let mut full = 0usize;
    let mut accepted = 0usize;
    for nonce in 0..4_000u64 {
        let tx = signed_transfer(&kp, recipient, 1, nonce, 1);
        match handle.submit(tx) {
            Ok(()) => accepted += 1,
            Err(aurion_node::node::IngressError::ChannelFull { .. }) => full += 1,
            Err(other) => panic!("galat tak terduga: {other}"),
        }
    }

    assert!(full > 0, "kanal berbatas harus menolak saat beban puncak");
    assert!(accepted > 0, "sebagian transaksi harus diterima");
    handle.shutdown();
}

/// O1: sumber actor tidak boleh membagi state dengan `Arc<Mutex<State>>`/
///_okupansi_ tunggal, melainkan lewat kanal.
#[test]
fn o1_actor_source_contains_no_shared_mutex_state() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/node.rs");
    let text = std::fs::read_to_string(&source).expect("baca src/node.rs");
    for forbidden in ["Arc<Mutex", "Arc< RwLock", "std::sync::Mutex"] {
        assert!(
            !text.contains(forbidden),
            "orchestration tidak boleh memakai {forbidden}"
        );
    }
    assert!(
        text.contains("mpsc::channel") || text.contains("tokio::sync::mpsc"),
        "harus memakai kanal async"
    );
}

// ---------------------------------------------------------------- O2

/// Bentukkan verdict blacklist dengan tanda tangan seluruh anggota guard.
fn unanimous_verdict(
    guards: &BTreeMap<PublicKeyBytes, Keypair>,
    target: PublicKeyBytes,
    violation: ViolationType,
) -> BlacklistVerdict {
    let evidence = RaidEvidence::new(target, violation, 12, &[0xEE; 32]);
    let mut signatures: BTreeMap<PublicKeyBytes, SignatureBytes> = BTreeMap::new();
    let digest = {
        let probe = BlacklistVerdict {
            evidence: evidence.clone(),
            signatures: BTreeMap::new(),
        };
        probe.digest()
    };
    for (pubkey, keypair) in guards {
        signatures.insert(*pubkey, keypair.sign(&digest));
    }
    BlacklistVerdict {
        evidence,
        signatures,
    }
}

/// O2: proposer yang diblokir guard tidak dapat menghasilkan blok.
#[tokio::test]
async fn o2_guard_blacklist_blocks_block_production() {
    let dir = test_dir("o2");
    let treasury = Keypair::generate();
    let proposer = Keypair::generate().public_key_bytes();

    let mut guards = BTreeMap::new();
    for _ in 0..5 {
        let kp = Keypair::generate();
        guards.insert(kp.public_key_bytes(), kp);
    }

    let mut config = config_with(&dir, treasury.public_key_bytes());
    config.initial_guards = guards.keys().copied().collect();
    config.block_proposer = proposer;
    config.initial_validators = vec![proposer];

    let mut runtime = NodeHandle::spawn(ChainNode::bootstrap(config).expect("bootstrap"));
    let handle = runtime.handle.clone();
    let recipient = Keypair::generate().public_key_bytes();

    handle
        .submit_verdict(unanimous_verdict(
            &guards,
            proposer,
            ViolationType::DoubleSigning,
        ))
        .expect("kirim putusan");
    let event = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::GuardVerdictApplied { .. })
    })
    .await;
    assert_eq!(
        event,
        ChainEvent::GuardVerdictApplied {
            target: proposer,
            blacklisted: true,
            tombstone_requested: true,
        },
        "putusan bulat double-signing harus diblokir sekaligus meminta tombstone"
    );

    handle
        .submit(signed_transfer(&treasury, recipient, 10, 0, 1))
        .expect("submit transaksi");
    handle.produce_block().expect("minta blok");

    let blocked = runtime.next_event_timeout(Duration::from_millis(400)).await;
    assert!(
        !matches!(blocked, Some(ChainEvent::BlockCommitted { .. })),
        "proposer terblokir tidak boleh mengomit blok"
    );

    let balance = handle.balance(recipient).await.expect("kueri saldo");
    assert_eq!(balance, None, "tidak ada transfer yang ter-commit");
    handle.shutdown();
}

/// O2: putative verdict tanpa tanda tangan bulat ditolak dan proposer tetap
/// berdaya.
#[tokio::test]
async fn o2_non_unanimous_verdict_is_rejected() {
    let dir = test_dir("o2-partial");
    let treasury = Keypair::generate();
    let proposer = Keypair::generate().public_key_bytes();

    let mut guards = BTreeMap::new();
    for _ in 0..5 {
        let kp = Keypair::generate();
        guards.insert(kp.public_key_bytes(), kp);
    }

    let mut config = config_with(&dir, treasury.public_key_bytes());
    config.initial_guards = guards.keys().copied().collect();
    config.block_proposer = proposer;
    config.initial_validators = vec![proposer];

    let mut runtime = NodeHandle::spawn(ChainNode::bootstrap(config).expect("bootstrap"));
    let handle = runtime.handle.clone();

    let mut verdict = unanimous_verdict(&guards, proposer, ViolationType::DoubleSigning);
    let dropped = *verdict.signatures.keys().next().expect("ada tanda tangan");
    verdict.signatures.remove(&dropped);

    handle.submit_verdict(verdict).expect("kirim putusan");
    let event = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::GuardVerdictApplied { .. })
    })
    .await;
    assert_eq!(
        event,
        ChainEvent::GuardVerdictApplied {
            target: proposer,
            blacklisted: false,
            tombstone_requested: false,
        },
        "verdict tanpa konsensus bulat harus ditolak"
    );
    handle.shutdown();
}

/// O2: pelanggaran liveness diblokir namun tidak memicu tombstone; hanya
/// pelanggaran akut (double-signing) yang memerintahkan karantina ireversibel.
#[tokio::test]
async fn o2_liveness_verdict_blacklists_without_tombstone() {
    let dir = test_dir("o2-liveness");
    let treasury = Keypair::generate();
    let proposer = Keypair::generate().public_key_bytes();

    let mut guards = BTreeMap::new();
    for _ in 0..5 {
        let kp = Keypair::generate();
        guards.insert(kp.public_key_bytes(), kp);
    }

    let mut config = config_with(&dir, treasury.public_key_bytes());
    config.initial_guards = guards.keys().copied().collect();
    config.block_proposer = proposer;
    config.initial_validators = vec![proposer];

    let mut runtime = NodeHandle::spawn(ChainNode::bootstrap(config).expect("bootstrap"));
    let handle = runtime.handle.clone();

    handle
        .submit_verdict(unanimous_verdict(
            &guards,
            proposer,
            ViolationType::UnresponsiveLivenessFailure,
        ))
        .expect("kirim putusan");
    let event = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::GuardVerdictApplied { .. })
    })
    .await;
    assert_eq!(
        event,
        ChainEvent::GuardVerdictApplied {
            target: proposer,
            blacklisted: true,
            tombstone_requested: false,
        },
        "pelanggaran liveness hanya memblokir, tidak mengarantina validator"
    );
    handle.shutdown();
}

// ---------------------------------------------------------------- O3

/// O3: frame P2P di-dekode codec sungguhan, lalu dikirim ke actor lewat
/// kanal gossip dan memengaruhi state konsensus.
#[tokio::test]
async fn o3_wire_frame_roundtrip_and_gossip_ingress() {
    let dir = test_dir("o3");
    let treasury = Keypair::generate();
    let validator = Keypair::generate();

    let mut config = config_with(&dir, treasury.public_key_bytes());
    config.initial_validators = vec![validator.public_key_bytes()];

    let mut runtime = NodeHandle::spawn(ChainNode::bootstrap(config).expect("bootstrap"));
    let handle = runtime.handle.clone();

    let vote = sign_vote(&validator, [0x11; 32], 1, 0, VoteType::Precommit);

    let frame = aurion_node::node::encode_gossip(&NetworkMessage::Vote(vote.clone()))
        .expect("encode frame");

    let decoded = decode_gossip(&frame)
        .expect("decode frame")
        .expect("frame tidak kosong");
    assert_eq!(
        decoded,
        NetworkMessage::Vote(vote.clone()),
        "roundtrip lossless"
    );

    handle
        .gossip(NetworkMessage::Vote(vote))
        .expect("kirim gossip");
    let event = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::VotesObserved { .. })
    })
    .await;
    assert_eq!(
        event,
        ChainEvent::VotesObserved {
            height: 1,
            votes: 1
        }
    );
    handle.shutdown();
}

/// O3: frame parsial ditolak codec, bukan ditelan diam-diam.
#[test]
fn o3_truncated_frame_is_rejected() {
    let frame = [0xAAu8; AurionWireCodec::HEADER_LEN + 1];
    assert!(
        decode_gossip(&frame).is_err(),
        "frame terpotong harus menghasilkan galat"
    );
    assert!(
        decode_gossip(&[]).expect("buffer kosong").is_none(),
        "buffer kosong berarti tidak ada pesan"
    );
}

// ---------------------------------------------------------------- O4

/// O3: *loopback mesh* tiga simpul semu dalam satu *runtime* memori. Suara BFT
/// dari simpul A disiarkan lewat frame jaringan, di-dekode `AurionWireCodec`,
/// lalu masuk ke mesin state konsensus simpul B dan C.
#[tokio::test]
async fn o3_three_node_loopback_mesh_propagates_votes() {
    let validators: Vec<Keypair> = (0..3).map(|_| Keypair::generate()).collect();
    let validator_keys: Vec<PublicKeyBytes> =
        validators.iter().map(Keypair::public_key_bytes).collect();

    let mut runtimes = Vec::new();
    for index in 0..3 {
        let treasury = Keypair::generate();
        let mut config = config_with(
            &test_dir(&format!("o3-mesh-{index}")),
            treasury.public_key_bytes(),
        );
        // Setiap simpul memvalidasi seluruh roster agar suara mana pun sah.
        config.initial_validators = validator_keys.clone();
        config.block_proposer = validator_keys[index];
        runtimes.push(NodeHandle::spawn(
            ChainNode::bootstrap(config).expect("bootstrap mesh"),
        ));
    }

    // Simpul A menyiarkan suara precommit-nya.
    let vote = sign_vote(&validators[0], [0x77; 32], 1, 0, VoteType::Precommit);
    runtimes[0]
        .handle
        .gossip(NetworkMessage::Vote(vote.clone()))
        .expect("siar dari A");
    let observed = wait_for(&mut runtimes[0], |e| {
        matches!(e, ChainEvent::VotesObserved { .. })
    })
    .await;
    assert_eq!(
        observed,
        ChainEvent::VotesObserved {
            height: 1,
            votes: 1
        }
    );

    // Jaringan: serialisasi di A, de-kode di B dan C.
    let frame = encode_gossip(&NetworkMessage::Vote(vote)).expect("encode frame mesh");
    for runtime in runtimes.iter_mut().skip(1) {
        let decoded = decode_gossip(&frame)
            .expect("decode frame mesh")
            .expect("frame mesh tidak kosong");
        runtime.handle.gossip(decoded).expect("terima di B/C");
        let observed = wait_for(runtime, |e| matches!(e, ChainEvent::VotesObserved { .. })).await;
        assert_eq!(
            observed,
            ChainEvent::VotesObserved {
                height: 1,
                votes: 1
            },
            "suara harus masuk ke state konsensus simpul penerima"
        );
    }

    for runtime in &runtimes {
        runtime.handle.shutdown();
    }
    for runtime in runtimes {
        runtime.join.await.expect("aktor berhenti");
    }
}

/// O3: *loopback mesh* empat simpul menuju kuorum BFT 3-of-4. Setiap simpul
/// terlibat penuh dalam satu penyiarannya sendiri (frame P2P di-encode lalu
/// di-dekode), dan seluruh suara precommit pada blok yang sama diterima oleh
/// semua simpul sehingga ambang `2f+1 = 3` tercapai di tiap mesin konsensus.
#[tokio::test]
async fn o3_four_node_mesh_reaches_bft_quorum() {
    const NODES: usize = 4;
    let validators: Vec<Keypair> = (0..NODES).map(|_| Keypair::generate()).collect();
    let validator_keys: Vec<PublicKeyBytes> =
        validators.iter().map(Keypair::public_key_bytes).collect();

    let mut runtimes = Vec::new();
    for index in 0..NODES {
        let treasury = Keypair::generate();
        let mut config = config_with(
            &test_dir(&format!("o3-mesh4-{index}")),
            treasury.public_key_bytes(),
        );
        // Seluruh roster diakui setiap simpul agar suara mana pun sah.
        config.initial_validators = validator_keys.clone();
        config.block_proposer = validator_keys[index];
        runtimes.push(NodeHandle::spawn(
            ChainNode::bootstrap(config).expect("bootstrap mesh4"),
        ));
    }

    // Satu blok kandidat bersama; tiap simpul menandatangani precommit-nya.
    let block_hash = [0x42; 32];
    let votes: Vec<Vote> = (0..NODES)
        .map(|index| sign_vote(&validators[index], block_hash, 1, 0, VoteType::Precommit))
        .collect();

    // Setiap simpul menyiarkan suaranya ke seluruh mesh melalui wire format.
    for vote in &votes {
        let frame = encode_gossip(&NetworkMessage::Vote(vote.clone())).expect("encode frame mesh4");
        for runtime in &mut runtimes {
            let decoded = decode_gossip(&frame)
                .expect("decode frame mesh4")
                .expect("frame mesh4 tidak kosong");
            runtime.handle.gossip(decoded).expect("terima di mesh4");
        }
    }

    // Kuorum 3-of-4 tercapai pada semua simpul: 4 suara sah diterima, tidak ada
    // duplikat, dan tidak ada simpul yang menolak suara simpang mana pun.
    for runtime in &mut runtimes {
        await_votes(runtime, 1, 4).await;
    }

    for runtime in &runtimes {
        runtime.handle.shutdown();
    }
    for runtime in runtimes {
        runtime.join.await.expect("aktor berhenti");
    }
}

/// O4: graceful shutdown melaporkan tinggi terakhir dan state tetap durable
/// setelah ledger ditutup.
#[tokio::test]
async fn o4_shutdown_is_graceful_and_state_is_durable() {
    let dir = test_dir("o4");
    let treasury = Keypair::generate();
    let mut config = config_with(&dir, treasury.public_key_bytes());

    let validator = Keypair::generate();
    config.initial_validators = vec![validator.public_key_bytes()];
    config.block_proposer = validator.public_key_bytes();

    let mut runtime = NodeHandle::spawn(ChainNode::bootstrap(config).expect("bootstrap"));
    let handle = runtime.handle.clone();
    let recipient = Keypair::generate().public_key_bytes();

    handle
        .submit(signed_transfer(&treasury, recipient, 25, 0, 1))
        .expect("submit");
    handle.produce_block().expect("minta blok");
    let committed = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::BlockCommitted { .. })
    })
    .await;
    let ChainEvent::BlockCommitted { height, .. } = committed else {
        panic!("harus blok ter-commit");
    };
    assert_eq!(height, 1);

    handle.shutdown();
    let stop = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::ShutdownComplete { .. })
    })
    .await;
    assert_eq!(stop, ChainEvent::ShutdownComplete { last_height: 1 });
    runtime.join.await.expect("actor berhenti bersih");

    // Durable check: ledger dibuka ulang setelah actor dilepas.
    let ledger = LedgerStore::open(&dir).expect("buka ulang");
    assert_eq!(ledger.get_latest_height().expect("tinggi"), 1);
    let block = ledger
        .get_block_by_height(1)
        .expect("q")
        .expect("blok 1 harus durable");
    assert_eq!(block.transactions.len(), 1);
    let _ = std::fs::remove_file(&dir);
}

/// O4: handle yang dipakai setelah shutdown ditolak, bukan diam-diam gagal.
#[tokio::test]
async fn o4_submit_after_shutdown_is_refused() {
    let dir = test_dir("o4-closed");
    let treasury = Keypair::generate();
    let node =
        ChainNode::bootstrap(config_with(&dir, treasury.public_key_bytes())).expect("bootstrap");
    let runtime = NodeHandle::spawn(node);
    let handle = runtime.handle.clone();

    handle.shutdown();
    runtime.join.await.expect("actor berhenti");

    let tx = signed_transfer(&treasury, Keypair::generate().public_key_bytes(), 1, 0, 1);
    assert_eq!(
        handle.submit(tx),
        Err(aurion_node::node::IngressError::NodeStopping)
    );
    assert_eq!(
        handle.produce_block(),
        Err(aurion_node::node::IngressError::NodeStopping)
    );
    let _ = std::fs::remove_file(&dir);
}

// ---------------------------------------------------------------- O5

/// O2: alur pipa eksekusi blok end-to-end — Gateway → Mempool → Guard →
/// `ExecutionEngine` (`WriteSet`) → `LedgerStore` → proyeksi, lalu kueri read
/// model memvalidasi bahwa mutasi state benar-benar tercermin.
#[tokio::test]
async fn o2_pipeline_commits_and_updates_read_model() {
    let dir = test_dir("o5");
    let treasury = Keypair::generate();
    let recipient = Keypair::generate().public_key_bytes();
    let mut config = config_with(&dir, treasury.public_key_bytes());

    let validator = Keypair::generate();
    config.initial_validators = vec![validator.public_key_bytes()];
    config.block_proposer = validator.public_key_bytes();

    let mut runtime = NodeHandle::spawn(ChainNode::bootstrap(config).expect("bootstrap"));
    let handle = runtime.handle.clone();

    let amount = 5_000u64;
    let fee = 7u64;
    let supply_before = handle
        .balance(treasury.public_key_bytes())
        .await
        .expect("kueri saldo awal")
        .expect("saldo genesis harus terbaca");

    handle
        .submit(signed_transfer(&treasury, recipient, amount, 0, fee))
        .expect("submit transaksi");
    let accepted = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::TransactionAccepted { .. })
    })
    .await;
    assert_eq!(
        accepted,
        ChainEvent::TransactionAccepted {
            height: 0,
            pending: 1
        }
    );

    handle.produce_block().expect("minta blok");
    let committed = wait_for(&mut runtime, |e| {
        matches!(e, ChainEvent::BlockCommitted { .. })
    })
    .await;
    let ChainEvent::BlockCommitted {
        height,
        tx_count,
        delta_entries,
        ..
    } = committed
    else {
        panic!("harus blok ter-commit");
    };
    assert_eq!(height, 1);
    assert_eq!(tx_count, 1);
    assert!(
        delta_entries > 0,
        "WriteSet eksekusi harus memuat entri saldo"
    );

    // Read model harus mencerminkan blok yang benar-benar dikomit.
    assert_eq!(
        handle.balance(recipient).await.expect("kueri saldo"),
        Some(amount)
    );
    // Treasury kehilangan tepat `amount + fee`; fee diteruskan ke proposer.
    let treasury_after = handle
        .balance(treasury.public_key_bytes())
        .await
        .expect("kueri saldo treasury")
        .expect("treasury ada");
    assert_eq!(treasury_after, supply_before - amount - fee);
    let proposer_balance = handle
        .balance(validator.public_key_bytes())
        .await
        .expect("kueri saldo proposer")
        .expect("proposer dibuat saat fee masuk");
    assert_eq!(proposer_balance, fee);

    handle.shutdown();
    runtime.join.await.expect("actor berhenti");
    let ledger = LedgerStore::open(&dir).expect("ledger");
    assert_eq!(ledger.get_latest_height().expect("tinggi"), 1);
    let _ = std::fs::remove_file(&dir);
}

/// O2: mempool yang kosong tidak menghasilkan blok kosong.
#[tokio::test]
async fn o2_empty_mempool_produces_no_block() {
    let dir = test_dir("o5-empty");
    let treasury = Keypair::generate();
    let mut runtime = NodeHandle::spawn(
        ChainNode::bootstrap(config_with(&dir, treasury.public_key_bytes())).expect("bootstrap"),
    );
    let handle = runtime.handle.clone();

    handle.produce_block().expect("minta blok");
    let event = runtime.next_event_timeout(Duration::from_millis(400)).await;
    assert!(
        !matches!(event, Some(ChainEvent::BlockCommitted { .. })),
        "mempool kosong tidak boleh menghasilkan blok"
    );
    handle.shutdown();
    runtime.join.await.expect("actor berhenti");
    let _ = std::fs::remove_file(&dir);
}

/// O5: audit leksikal *zero-float* di seluruh lapisan integrasi aplikasi.
/// Tidak boleh ada satupun `f32`/`f64` di `apps/aurion-node/src/`.
#[test]
fn o5_no_float_representation_in_node_sources() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&src)
        .expect("baca direktori src")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "harus ada sumber Rust untuk dipindai");

    for file in &files {
        let text = std::fs::read_to_string(file).expect("baca sumber");
        for line in text.lines() {
            let code = line.split("//").next().unwrap_or(line);
            for token in ["f32", "f64"] {
                assert!(
                    !code.contains(token),
                    "{} memuat representasi float `{token}`: {}",
                    file.display(),
                    line.trim()
                );
            }
        }
    }
}

/// O5: parameter konfigurasi aplikasi dibaca sebagai bilangan bulat/string,
/// bukan pecahan. Divisors dihitung dengan bilangan bulat.
#[test]
fn o5_config_uses_integer_arithmetic_for_saturation() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/node.rs");
    let text = std::fs::read_to_string(source).expect("baca src/node.rs");
    assert!(
        text.contains("PREFIX_BALANCE.len()"),
        "perhitungan panjang kunci harus memakai panjang prefix"
    );
    assert!(
        text.contains("NAMESPACE_DIGEST_LEN"),
        "offsets kunci harus memakai konstanta namespace"
    );
    assert!(
        !text.contains("as f64") && !text.contains("as f32"),
        "tidak boleh ada konversi ke floating-point"
    );
}
