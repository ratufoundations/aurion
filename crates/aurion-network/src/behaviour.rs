//! Behaviour libp2p terisolasi: Gossipsub + Identify di balik facade domain.
//!
//! Modul ini adalah satu-satunya tempat tipe mentah `libp2p` diizinkan muncul
//! di dalam `aurion-network`. Lapisan luar hanya melihat [`InboundGossip`]
//! (topic hash string + payload bita) sehingga tidak ada tipe float/scoring
//! libp2p yang bocor ke modul bisnis.

use crate::{error::NetworkError, message::MAX_FRAME_SIZE};
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identify,
    swarm::NetworkBehaviour,
};

/// Batas transmisi Gossipsub: selaras dengan batas frame wire (4 MiB).
pub const MAX_TRANSMIT_SIZE: usize = MAX_FRAME_SIZE;
/// Topik gossip transaksi domain Aurion.
pub const GOSSIP_TOPIC_TXS: &str = "aurion/tx/v1";
/// Topik gossip blok domain Aurion.
pub const GOSSIP_TOPIC_BLOCKS: &str = "aurion/blocks/v1";
/// Topik gossip voting konsensus domain Aurion.
pub const GOSSIP_TOPIC_CONSENSUS: &str = "aurion/consensus/v1";

/// Pesan gossip masuk yang sudah difilter ke tipe domain murni.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundGossip {
    pub topic: String,
    pub payload: Vec<u8>,
}

/// Gabungan behaviour Gossipsub + Identify untuk satu `Swarm` Aurion.
#[allow(missing_debug_implementations)]
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "AurionBehaviourOutEvent")]
pub struct AurionBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
}

/// Event domain yang keluar dari `Swarm`: hanya tipe Aurion murni.
#[derive(Debug)]
pub enum AurionBehaviourOutEvent {
    Gossipsub(Box<gossipsub::Event>),
    Identify(Box<identify::Event>),
}

impl From<gossipsub::Event> for AurionBehaviourOutEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::Gossipsub(Box::new(event))
    }
}

impl From<identify::Event> for AurionBehaviourOutEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(Box::new(event))
    }
}

/// Hash topik gossip untuk satu nama topik domain.
#[must_use]
pub fn gossip_topic_hash(topic: &str) -> gossipsub::TopicHash {
    IdentTopic::new(topic).hash()
}

/// Bangun behaviour standar Aurion: Gossipsub terikat `MAX_TRANSMIT_SIZE`
/// dengan validasi ketat, plus Identify untuk negosiasi protokol/chain.
///
/// # Errors
/// Mengembalikan `NetworkError::Io` bila konfigurasi gossipsub ditolak
/// (misalnya ukuran transmisi tidak valid), atau error berantai bila
/// pembuatan Keypair Noise/Identify gagal.
pub fn build_behaviour(
    local_key: &libp2p::identity::Keypair,
) -> Result<AurionBehaviour, NetworkError> {
    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .max_transmit_size(MAX_TRANSMIT_SIZE)
        .validation_mode(ValidationMode::Strict)
        .build()
        .map_err(|err| NetworkError::Io(err.to_string()))?;
    let mut gossipsub = gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )
    .map_err(|err| NetworkError::Io(err.to_string()))?;
    for topic in [
        GOSSIP_TOPIC_TXS,
        GOSSIP_TOPIC_BLOCKS,
        GOSSIP_TOPIC_CONSENSUS,
    ] {
        gossipsub
            .subscribe(&IdentTopic::new(topic))
            .map_err(|err| NetworkError::Io(err.to_string()))?;
    }
    let identify_config =
        identify::Config::new("/aurion/identify/1.0.0".to_owned(), local_key.public());
    let identify = identify::Behaviour::new(identify_config);
    Ok(AurionBehaviour {
        gossipsub,
        identify,
    })
}

/// Saring pesan gossip mentah menjadi payload domain: tolak payload kosong
/// atau melebihi `MAX_TRANSMIT_SIZE` sebelum masuk codec Aurion.
///
/// # Errors
/// Mengembalikan `FrameTooLarge` bila payload melebihi batas, atau
/// `MalformedPayload` bila payload kosong.
pub fn filter_inbound_gossip(
    topic: &gossipsub::TopicHash,
    payload: &[u8],
) -> Result<InboundGossip, NetworkError> {
    if payload.len() > MAX_TRANSMIT_SIZE {
        return Err(NetworkError::FrameTooLarge {
            size: payload.len(),
            max_allowed: MAX_TRANSMIT_SIZE,
        });
    }
    if payload.is_empty() {
        return Err(NetworkError::MalformedPayload);
    }
    Ok(InboundGossip {
        topic: topic.to_string(),
        payload: payload.to_vec(),
    })
}
