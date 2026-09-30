#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use aurion_consensus::{Vote, VoteType};
use aurion_core::{Block, BlockHeader, Transaction};
use aurion_criptografi::Keypair;
use aurion_network::{
    build_behaviour, filter_inbound_gossip, gossip_topic_hash, handshake_deadline_ms,
    AurionWireCodec, AuthenticatedGate, BandwidthMeter, Handshake, HandshakeStatus, NetworkCommand,
    NetworkError, NetworkMessage, NetworkService, PeerStatus, RttTracker, HANDSHAKE_TIMEOUT_MS,
    KEEPALIVE_PING_INTERVAL_MS, MAX_FRAME_SIZE, MAX_RTT_MS, MAX_TRANSMIT_SIZE, PROTOCOL_VERSION,
};
use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

fn key(seed: u8) -> Keypair {
    Keypair::from_bytes(&[seed; 32])
}

fn signed_tx(
    sender: &Keypair,
    recipient: [u8; 32],
    amount: u128,
    nonce: u64,
    fee: u128,
) -> Transaction {
    let pk = sender.public_key_bytes();
    let unsigned = Transaction::new(pk, recipient, amount, nonce, fee, [0; 64]);
    Transaction::new(
        pk,
        recipient,
        amount,
        nonce,
        fee,
        sender.sign(&unsigned.digest()),
    )
}

fn encode(msg: NetworkMessage) -> Vec<u8> {
    let mut codec = AurionWireCodec;
    let mut buf = BytesMut::new();
    codec.encode(msg, &mut buf).expect("encode valid");
    buf.to_vec()
}

fn decode(bytes: &[u8]) -> Result<Option<NetworkMessage>, NetworkError> {
    AurionWireCodec.decode(&mut BytesMut::from(bytes))
}

fn handshake(node: [u8; 32], chain_id: u64, version: u16) -> NetworkMessage {
    NetworkMessage::Handshake(Handshake {
        node_id: node,
        chain_id,
        protocol_version: version,
        listen_port: 8080,
    })
}

#[test]
fn n0_wire_codec_round_trips_losslessly_and_deterministically() {
    let alice = key(1);
    let bob = key(2);
    let tx = signed_tx(&alice, bob.public_key_bytes(), 50_000, 7, 3);
    let tx_msg = NetworkMessage::Transaction(tx.clone());
    assert_eq!(
        decode(&encode(tx_msg.clone()))
            .expect("decode tx")
            .expect("some"),
        tx_msg
    );
    assert_eq!(encode(tx_msg.clone()), encode(tx_msg.clone()));
    let voter = key(3);
    let unsigned_vote = Vote::new(
        voter.public_key_bytes(),
        [0xAA; 32],
        100,
        2,
        VoteType::Precommit,
        [0; 64],
    );
    let sig = voter.sign(&unsigned_vote.digest());
    let vote = Vote::new(
        voter.public_key_bytes(),
        [0xAA; 32],
        100,
        2,
        VoteType::Precommit,
        sig,
    );
    let vote_msg = NetworkMessage::Vote(vote.clone());
    assert_eq!(
        decode(&encode(vote_msg.clone()))
            .expect("decode vote")
            .expect("some"),
        vote_msg
    );
    assert_eq!(encode(vote_msg.clone()), encode(vote_msg.clone()));
    let block = Block {
        header: BlockHeader {
            height: 9,
            prev_hash: [9; 32],
            state_root: [8; 32],
            tx_count: 1,
            timestamp: 9_000,
            proposer: [7; 32],
        },
        transactions: vec![tx],
    };
    let block_msg = NetworkMessage::Block(block.clone());
    assert_eq!(
        decode(&encode(block_msg.clone()))
            .expect("decode block")
            .expect("some"),
        block_msg
    );
    for nonce in [1u64, 2, 99] {
        let echo_request = NetworkMessage::Ping(nonce);
        assert_eq!(
            decode(&encode(echo_request.clone()))
                .expect("decode ping")
                .expect("some"),
            echo_request
        );
        let echo_reply = NetworkMessage::Pong(nonce);
        assert_eq!(
            decode(&encode(echo_reply.clone()))
                .expect("decode pong")
                .expect("some"),
            echo_reply
        );
    }
    let hs_msg = handshake([0x11; 32], 1001, PROTOCOL_VERSION);
    assert_eq!(
        decode(&encode(hs_msg.clone()))
            .expect("decode hs")
            .expect("some"),
        hs_msg
    );
}

#[test]
fn n1_oversized_frame_claims_are_rejected_without_heap_exhaustion() {
    assert_eq!(MAX_TRANSMIT_SIZE, MAX_FRAME_SIZE);
    assert_eq!(MAX_FRAME_SIZE, 4 * 1024 * 1024);
    let mut evil = BytesMut::new();
    evil.extend_from_slice(&aurion_network::AURION_NET_MAGIC.to_be_bytes());
    evil.extend_from_slice(&u32::MAX.to_be_bytes());
    evil.extend_from_slice(&[0x02]);
    let before = evil.len();
    let err = AurionWireCodec
        .decode(&mut evil)
        .expect_err("must reject giant claim");
    assert_eq!(
        err,
        NetworkError::FrameTooLarge {
            size: u32::MAX as usize,
            max_allowed: MAX_FRAME_SIZE
        }
    );
    assert!(
        evil.len() >= before - 1,
        "decoder must not consume giant payload"
    );
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let behaviour = build_behaviour(&local_key).expect("behaviour builds");
    let _ = behaviour;
    let topic = gossip_topic_hash(aurion_network::GOSSIP_TOPIC_TXS);
    let giant = vec![0xABu8; MAX_TRANSMIT_SIZE + 1];
    assert_eq!(
        filter_inbound_gossip(&topic, &giant),
        Err(NetworkError::FrameTooLarge {
            size: giant.len(),
            max_allowed: MAX_TRANSMIT_SIZE
        })
    );
}

#[test]
fn n2_handshake_chain_and_version_gate_and_first_message_rule() {
    let good = Handshake {
        node_id: [1; 32],
        chain_id: 1001,
        protocol_version: PROTOCOL_VERSION,
        listen_port: 8080,
    };
    assert_eq!(good.validate(1001), Ok(HandshakeStatus::Accepted));
    assert_eq!(
        good.validate(9999),
        Err(NetworkError::ChainIdMismatch {
            expected: 9999,
            got: 1001
        })
    );
    let stale = Handshake {
        node_id: [1; 32],
        chain_id: 1001,
        protocol_version: 0,
        listen_port: 8080,
    };
    assert_eq!(
        stale.validate(1001),
        Err(NetworkError::IncompatibleProtocolVersion {
            expected: PROTOCOL_VERSION,
            got: 0
        })
    );
    let mut gate = AuthenticatedGate::new(1001);
    let early_tx = NetworkMessage::Transaction(signed_tx(&key(10), [20; 32], 5, 0, 1));
    assert_eq!(
        gate.admit(&early_tx),
        Err(NetworkError::UnauthenticatedMessage)
    );
    assert!(!gate.is_authenticated());
    assert_eq!(
        gate.admit(&handshake([1; 32], 9999, PROTOCOL_VERSION)),
        Err(NetworkError::ChainIdMismatch {
            expected: 1001,
            got: 9999
        })
    );
    assert!(!gate.is_authenticated());
    assert_eq!(
        gate.admit(&handshake([1; 32], 1001, PROTOCOL_VERSION)),
        Ok(Some(HandshakeStatus::Accepted))
    );
    assert!(gate.is_authenticated());
    assert_eq!(gate.admit(&early_tx), Ok(None));
}

#[test]
fn n3_malformed_truncated_and_random_streams_never_panic() {
    let mut bad_magic = BytesMut::from([0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 1, 0x05].as_slice());
    assert!(matches!(
        AurionWireCodec.decode(&mut bad_magic),
        Err(NetworkError::InvalidMagicBytes { .. })
    ));
    let full = encode(NetworkMessage::Ping(0x0102_0304_0506_0708));
    let cut = full.len() / 2;
    assert_eq!(
        AurionWireCodec::decode_at_eof(&full[..cut]),
        Err(NetworkError::UnexpectedEof)
    );
    assert_eq!(AurionWireCodec::decode_at_eof(&[]), Ok(None));
    assert_eq!(
        AurionWireCodec::decode_at_eof(&full)
            .expect("full eof")
            .expect("some"),
        NetworkMessage::Ping(0x0102_0304_0506_0708)
    );
    let mut state: u64 = 0x1234_5678_9ABC_DEF1;
    let mut panics = 0u32;
    for _ in 0..1_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let len = (state >> 33) as usize % 96;
        let mut chunk = Vec::with_capacity(len);
        let mut word = state;
        for _ in 0..len {
            word = word
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let byte = u8::try_from((word >> 33) & 0xFF).expect("low byte fits");
            chunk.push(byte);
        }
        let outcome = std::panic::catch_unwind(|| {
            let mut buf = BytesMut::from(chunk.as_slice());
            let _ = AurionWireCodec.decode(&mut buf);
        });
        if outcome.is_err() {
            panics += 1;
        }
    }
    assert_eq!(panics, 0, "decoder must never panic on fuzzed bytes");
}

#[tokio::test]
async fn n4_peer_fault_is_isolated_and_mesh_stays_live() {
    use aurion_network::PeerConnection;
    use tokio::net::{TcpListener, TcpStream};
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let mut live = 0u32;
        for _ in 0..2 {
            let (socket, _) = listener.accept().await.expect("accept");
            let mut peer = PeerConnection::new(socket);
            if let Ok(Some(NetworkMessage::Ping(n))) = peer.read_message().await {
                peer.send_message(NetworkMessage::Pong(n))
                    .await
                    .expect("pong");
                live += 1;
            }
        }
        live
    });
    let mut peer_a = PeerConnection::new(TcpStream::connect(addr).await.expect("connect a"));
    let mut peer_c = PeerConnection::new(TcpStream::connect(addr).await.expect("connect c"));
    peer_a
        .send_message(NetworkMessage::Ping(11))
        .await
        .expect("ping a");
    assert_eq!(
        peer_a.read_message().await.expect("read a").expect("some"),
        NetworkMessage::Pong(11)
    );
    drop(peer_a);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    peer_c
        .send_message(NetworkMessage::Ping(33))
        .await
        .expect("ping c");
    assert_eq!(
        peer_c.read_message().await.expect("read c").expect("some"),
        NetworkMessage::Pong(33)
    );
    assert_eq!(server.await.expect("server"), 2);
    let mut svc = NetworkService::new(1001);
    svc.apply_command(NetworkCommand::NotePeerConnected {
        peer: "A".to_owned(),
    });
    svc.apply_command(NetworkCommand::NotePeerConnected {
        peer: "B".to_owned(),
    });
    svc.apply_command(NetworkCommand::NotePeerConnected {
        peer: "C".to_owned(),
    });
    svc.apply_command(NetworkCommand::NotePeerDisconnected {
        peer: "B".to_owned(),
    });
    assert_eq!(svc.mesh().status("B"), Some(PeerStatus::Disconnected));
    assert_eq!(
        svc.mesh().broadcast_targets(None),
        vec!["A".to_owned(), "C".to_owned()]
    );
}

#[test]
fn n5_metrics_are_integer_only_checked_and_bounded() {
    assert_eq!(HANDSHAKE_TIMEOUT_MS, 5_000);
    assert_eq!(KEEPALIVE_PING_INTERVAL_MS, 1_000);
    assert_eq!(MAX_RTT_MS, 30_000);
    assert_eq!(
        handshake_deadline_ms(1_000, HANDSHAKE_TIMEOUT_MS),
        Some(6_000)
    );
    assert_eq!(handshake_deadline_ms(u64::MAX, 1), None);
    let mut meter = BandwidthMeter::new();
    assert!(meter.record_sent(1_000));
    assert!(meter.record_received(u64::MAX));
    assert!(!meter.record_received(1));
    assert_eq!(meter.total_bytes(), None);
    let mut ok_meter = BandwidthMeter::new();
    assert!(ok_meter.record_sent(40));
    assert!(ok_meter.record_received(2));
    assert_eq!(ok_meter.total_bytes(), Some(42));
    let mut rtt = RttTracker::new(100);
    assert_eq!(rtt.observe(200), Some(125));
    assert_eq!(rtt.observe(u64::MAX), None);
    assert_eq!(rtt.ema_ms(), 125);
    let src = include_str!("../src/metrics.rs");
    assert!(
        !src.contains("f32") && !src.contains("f64"),
        "metrics module must stay zero-float"
    );
}
