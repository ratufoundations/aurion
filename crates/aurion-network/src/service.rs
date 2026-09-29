//! Facade `NetworkService`: satu-satunya pintu masuk/keluar domain.
//!
//! Tipe mentah `libp2p` tidak pernah bocor dari modul ini: ke luar hanya ada
//! perintah integer-safe, frame domain yang lolos filter ukuran + decode wire.

use crate::{
    behaviour::{self, GOSSIP_TOPIC_BLOCKS, GOSSIP_TOPIC_CONSENSUS, GOSSIP_TOPIC_TXS},
    message::{NetworkMessage, MAX_FRAME_SIZE},
    metrics::{BandwidthMeter, RttTracker},
    peer::{AuthenticatedGate, PeerStatus},
    AurionWireCodec, NetworkError,
};
use bytes::BytesMut;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Encoder};

/// Perintah domain ke facade jaringan: semuanya integer-safe (`u64`/bita).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkCommand {
    Publish { topic: String, payload: Vec<u8> },
    NotePeerConnected { peer: String },
    NotePeerDisconnected { peer: String },
    ObserveRtt { peer: String, sample_ms: u64 },
}

/// Frame masuk yang sudah lolos filter ukuran dan decode wire codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainInbound {
    pub topic: String,
    pub message: NetworkMessage,
}

/// Frame keluar yang sudah lolos batas ukuran dan encode wire codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainOutbound {
    pub topic: String,
    pub frame: Vec<u8>,
    pub frame_len: u64,
}

/// Status mesh gossip per peer: terhubung atau terputus bersih.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MeshView {
    peers: HashMap<String, PeerStatus>,
}

impl MeshView {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn note_connected(&mut self, peer: &str) {
        self.peers.insert(peer.to_owned(), PeerStatus::Connected);
    }
    pub fn note_disconnected(&mut self, peer: &str) {
        self.peers.insert(peer.to_owned(), PeerStatus::Disconnected);
    }
    #[must_use]
    pub fn status(&self, peer: &str) -> Option<PeerStatus> {
        self.peers.get(peer).copied()
    }
    #[must_use]
    pub fn connected_peers(&self) -> Vec<String> {
        let mut peers: Vec<String> = self
            .peers
            .iter()
            .filter_map(|(peer, status)| {
                if *status == PeerStatus::Connected {
                    Some(peer.clone())
                } else {
                    None
                }
            })
            .collect();
        peers.sort();
        peers
    }
    /// Siaran ke mesh hanya ke peer yang masih terhubung (isolasi fault N4).
    #[must_use]
    pub fn broadcast_targets(&self, exclude: Option<&str>) -> Vec<String> {
        self.connected_peers()
            .into_iter()
            .filter(|peer| Some(peer.as_str()) != exclude)
            .collect()
    }
}

/// Facade jaringan domain: filter ukuran gossip, decode wire, guard handshake,
/// mesh peer, dan metrik integer -- tanpa membocorkan tipe `libp2p`.
#[derive(Debug)]
pub struct NetworkService {
    expected_chain_id: u64,
    gates: HashMap<String, AuthenticatedGate>,
    mesh: MeshView,
    topics: HashSet<String>,
    bandwidth: BandwidthMeter,
    rtt: HashMap<String, RttTracker>,
}

impl NetworkService {
    #[must_use]
    pub fn new(expected_chain_id: u64) -> Self {
        Self {
            expected_chain_id,
            gates: HashMap::new(),
            mesh: MeshView::new(),
            topics: HashSet::from([
                GOSSIP_TOPIC_TXS.to_owned(),
                GOSSIP_TOPIC_BLOCKS.to_owned(),
                GOSSIP_TOPIC_CONSENSUS.to_owned(),
            ]),
            bandwidth: BandwidthMeter::new(),
            rtt: HashMap::new(),
        }
    }
    #[must_use]
    pub const fn expected_chain_id(&self) -> u64 {
        self.expected_chain_id
    }
    #[must_use]
    pub fn mesh(&self) -> &MeshView {
        &self.mesh
    }
    #[must_use]
    pub fn bandwidth(&self) -> &BandwidthMeter {
        &self.bandwidth
    }
    #[must_use]
    pub fn rtt_ms(&self, peer: &str) -> Option<u64> {
        self.rtt.get(peer).map(RttTracker::ema_ms)
    }
    fn gate_for(&mut self, peer: &str) -> &mut AuthenticatedGate {
        self.gates
            .entry(peer.to_owned())
            .or_insert_with(|| AuthenticatedGate::new(self.expected_chain_id))
    }
    /// Tangani satu frame gossip mentah untuk satu peer: filter ukuran,
    /// decode wire, lalu guard handshake sebelum masuk domain.
    ///
    /// # Errors
    /// Mengembalikan error wire tanpa panic.
    pub fn handle_inbound_frame(
        &mut self,
        peer: &str,
        topic: &str,
        payload: &[u8],
    ) -> Result<Option<DomainInbound>, NetworkError> {
        if !self.topics.contains(topic) {
            return Err(NetworkError::UnknownMessageType(0xFF));
        }
        let topic_hash = behaviour::gossip_topic_hash(topic);
        let filtered = behaviour::filter_inbound_gossip(&topic_hash, payload)?;
        let _ = self
            .bandwidth
            .record_received(filtered.payload.len() as u64);
        let decoded = Self::decode_wire(&filtered.payload)?;
        if self.mesh.status(peer).is_none() {
            self.mesh.note_connected(peer);
        }
        let gate = self.gate_for(peer);
        gate.admit(&decoded)?;
        Ok(Some(DomainInbound {
            topic: filtered.topic,
            message: decoded,
        }))
    }
    /// Bangun frame keluar domain: encode wire + batas ukuran + akuntansi.
    ///
    /// # Errors
    /// Mengembalikan `FrameTooLarge` atau `UnknownMessageType`.
    pub fn build_outbound(
        &mut self,
        topic: &str,
        message: NetworkMessage,
    ) -> Result<DomainOutbound, NetworkError> {
        if !self.topics.contains(topic) {
            return Err(NetworkError::UnknownMessageType(0xFF));
        }
        let mut codec = AurionWireCodec;
        let mut buf = BytesMut::new();
        codec.encode(message, &mut buf)?;
        let frame = buf.to_vec();
        let _ = self.bandwidth.record_sent(frame.len() as u64);
        let frame_len = u64::try_from(frame.len()).map_err(|_| NetworkError::FrameTooLarge {
            size: frame.len(),
            max_allowed: MAX_FRAME_SIZE,
        })?;
        Ok(DomainOutbound {
            topic: topic.to_owned(),
            frame,
            frame_len,
        })
    }
    fn decode_wire(payload: &[u8]) -> Result<NetworkMessage, NetworkError> {
        let mut buf = BytesMut::from(payload);
        let mut codec = AurionWireCodec;
        match codec.decode(&mut buf)? {
            Some(msg) => {
                if !buf.is_empty() {
                    return Err(NetworkError::MalformedPayload);
                }
                Ok(msg)
            }
            None => Err(NetworkError::UnexpectedEof),
        }
    }
    /// Terapkan perintah domain (mesh + RTT integer-safe).
    pub fn apply_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::Publish { .. } => {}
            NetworkCommand::NotePeerConnected { peer } => {
                self.mesh.note_connected(&peer);
            }
            NetworkCommand::NotePeerDisconnected { peer } => {
                self.mesh.note_disconnected(&peer);
                self.gates.remove(&peer);
            }
            NetworkCommand::ObserveRtt { peer, sample_ms } => {
                self.rtt
                    .entry(peer)
                    .or_insert_with(|| RttTracker::new(sample_ms))
                    .observe(sample_ms);
            }
        }
    }
}

/// Handle kanal asinkron facade: domain hanya bertukar tipe Aurion murni.
#[derive(Debug)]
pub struct NetworkServiceHandle {
    pub commands: mpsc::Sender<NetworkCommand>,
    pub inbound: mpsc::Receiver<DomainInbound>,
    pub outbound: mpsc::Receiver<DomainOutbound>,
}

impl NetworkServiceHandle {
    #[must_use]
    pub fn channel(buffer: usize) -> (mpsc::Sender<NetworkCommand>, Self) {
        let (command_tx, _command_rx) = mpsc::channel(buffer);
        let (_inbound_tx, inbound_rx) = mpsc::channel(buffer);
        let (_outbound_tx, outbound_rx) = mpsc::channel(buffer);
        let handle = Self {
            commands: command_tx.clone(),
            inbound: inbound_rx,
            outbound: outbound_rx,
        };
        (command_tx, handle)
    }
}
