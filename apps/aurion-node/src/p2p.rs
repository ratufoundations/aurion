//! Driver jaringan P2P in-daemon: koneksi TCP nyata di atas wire codec Aurion.
//!
//! Peran modul ini adalah peserta mesh: mendengarkan `p2p_bind_addr`,
//! mendial `seed_nodes`, menjalankan gerbang handshake [`AuthenticatedGate`],
//! memancarkan ulang (relay) gossip antar-peer dengan batas `seen` agar tidak
//! berputar, dan meneruskan frame yang lolos gerbang ke actor rantai lewat
//! [`NodeHandle::gossip`]. Driver tidak pernah memegang state rantai; ia hanya
//! memindahkan [`NetworkMessage`] terenkode wire.
#![allow(
    clippy::module_name_repetitions,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::time::Duration;

use aurion_criptografi::PublicKeyBytes;
use aurion_network::{
    AuthenticatedGate, Handshake, NetworkMessage, PeerConnection, PROTOCOL_VERSION,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::node::{encode_gossip, IngressError, NodeHandle};

/// Kapasitas kanal perintah ke driver.
const COMMAND_CHANNEL_CAPACITY: usize = 64;
/// Kapasitas kanal peristiwa koneksi dari seluruh task peer.
const EVENT_CHANNEL_CAPACITY: usize = 256;
/// Kapasitas kanal keluar per koneksi peer (backpressure alih-alih buffer).
const PEER_CHANNEL_CAPACITY: usize = 64;
/// Batas ring `seen`; saat penuh, sidik lama dikeluarkan.
const SEEN_LIMIT: usize = 1024;
/// Interval percobaan redial ke seed yang belum tercapai.
const REDIAL_INTERVAL: Duration = Duration::from_millis(1_500);

/// Konfigurasi driver mesh P2P.
#[derive(Debug, Clone)]
pub struct P2PConfig {
    /// Identitas simpul (kunci penanda tangan handshake & suara BFT).
    pub node_id: PublicKeyBytes,
    /// Identitas jaringan; handshake dari peer disetujui hanya bila cocok.
    pub chain_id: u64,
    /// Alamat listen (`host:port`).
    pub listen_addr: String,
    /// Batas koneksi peer simultan.
    pub max_peers: usize,
    /// Alamat peer yang didial saat startup dan diredial saat putus.
    pub seeds: Vec<String>,
}

/// Perintah ke tujuan driver dari daemon.
#[derive(Debug)]
pub enum P2pCommand {
    /// Siarkan satu pesan ke seluruh peer terhubung.
    Broadcast(NetworkMessage),
    /// Kueri banyak peer yang sedang terhubung.
    PeerCountQuery { reply: oneshot::Sender<usize> },
}

/// Peristiwa dari task koneksi ke loop kendali mesh.
#[derive(Debug)]
enum PeerEvent {
    /// Koneksi baru siap (`link` dipakai mengirim keluar).
    Connected {
        peer: String,
        link: mpsc::Sender<NetworkMessage>,
    },
    /// Satu frame terdekode tiba dari peer.
    Message {
        peer: String,
        message: NetworkMessage,
    },
    /// Koneksi peer ditutup.
    Disconnected { peer: String },
}

/// Inti mesh: pemilik registri peer, gerbang autentikasi, dan ring `seen`.
struct MeshDriver {
    config: P2PConfig,
    actor: NodeHandle,
    event_tx: mpsc::Sender<PeerEvent>,
    listen_port: u16,
    peers: HashMap<String, mpsc::Sender<NetworkMessage>>,
    gates: HashMap<String, AuthenticatedGate>,
    seen: VecDeque<[u8; 32]>,
    seen_set: HashSet<[u8; 32]>,
}

impl MeshDriver {
    fn new(
        config: P2PConfig,
        actor: NodeHandle,
        event_tx: mpsc::Sender<PeerEvent>,
        listen_port: u16,
    ) -> Self {
        Self {
            config,
            actor,
            event_tx,
            listen_port,
            peers: HashMap::new(),
            gates: HashMap::new(),
            seen: VecDeque::new(),
            seen_set: HashSet::new(),
        }
    }

    /// Siapkan `link` keluar untuk peer baru dan antrekan handshake kita.
    /// Mengembalikan `false` bila peer duplikat sehingga koneksinya dibuang.
    fn register_peer(&mut self, peer: &str, link: mpsc::Sender<NetworkMessage>) -> bool {
        if self.peers.contains_key(peer) {
            return false;
        }
        self.gates.insert(
            peer.to_owned(),
            AuthenticatedGate::new(self.config.chain_id),
        );
        let handshake = NetworkMessage::Handshake(Handshake {
            node_id: self.config.node_id,
            chain_id: self.config.chain_id,
            protocol_version: PROTOCOL_VERSION,
            listen_port: self.listen_port,
        });
        if link.try_send(handshake).is_err() {
            self.note_disconnected(peer);
            return false;
        }
        self.peers.insert(peer.to_owned(), link);
        tracing::info!(peer, "Peer terhubung, autentikasi dimulai");
        true
    }

    fn note_disconnected(&mut self, peer: &str) {
        self.peers.remove(peer);
        self.gates.remove(peer);
        tracing::info!(peer, "Peer terputus");
    }

    fn connected_count(&self) -> usize {
        self.peers.len()
    }

    /// Kirim satu pesan ke satu peer terhubung (gagal = dibuang).
    fn send_to(&self, peer: &str, message: NetworkMessage) {
        if let Some(link) = self.peers.get(peer) {
            let _ = link.try_send(message);
        }
    }

    /// Siarkan pesan ke seluruh mesh.
    fn broadcast(&self, message: &NetworkMessage) {
        for link in self.peers.values() {
            let _ = link.try_send(message.clone());
        }
    }

    /// Teruskan frame yang lolos gerbang ke actor rantai.
    fn forward_to_actor(&self, message: &NetworkMessage) {
        if let Err(IngressError::ChannelFull { .. }) = self.actor.gossip(message.clone()) {
            tracing::warn!("Gossip masuk dibuang: kanal actor penuh");
        }
    }

    /// Relay satu pesan ke seluruh peer selain asalnya, tanpa loop (ring seen).
    fn relay(&mut self, from: &str, message: &NetworkMessage) {
        let targets: Vec<String> = self
            .peers
            .iter()
            .filter(|(peer, _)| peer.as_str() != from)
            .map(|(peer, _)| peer.clone())
            .collect();
        for target in targets {
            self.send_to(&target, message.clone());
        }
    }

    /// Sidik pesan untuk pencegahan loop; `True` bila pesan baru (belum dilihat).
    fn mark_seen(&mut self, message: &NetworkMessage) -> bool {
        let fingerprint = blake3::hash(&encode_gossip(message).unwrap_or_default()).into();
        if self.seen_set.contains(&fingerprint) {
            return false;
        }
        self.seen_set.insert(fingerprint);
        self.seen.push_back(fingerprint);
        while self.seen.len() > SEEN_LIMIT {
            if let Some(old) = self.seen.pop_front() {
                self.seen_set.remove(&old);
            }
        }
        true
    }

    /// Tangani satu pesan masuk dari peer sesuai gerbang autentikasi.
    fn handle_peer_message(&mut self, peer: &str, message: &NetworkMessage) {
        let Some(gate) = self.gates.get_mut(peer) else {
            self.note_disconnected(peer);
            return;
        };
        match gate.admit(message) {
            Ok(Some(_)) => {
                tracing::info!(peer, "Handshake peer tervalidasi");
            }
            Ok(None) => match message {
                NetworkMessage::Ping(nonce) => {
                    self.send_to(peer, NetworkMessage::Pong(*nonce));
                }
                NetworkMessage::Handshake(_) | NetworkMessage::Pong(_) => {}
                NetworkMessage::Transaction(_)
                | NetworkMessage::Block(_)
                | NetworkMessage::Vote(_) => {
                    if !self.mark_seen(message) {
                        return;
                    }
                    self.forward_to_actor(message);
                    self.relay(peer, message);
                }
            },
            Err(error) => {
                tracing::warn!(peer, error = %error, "Frame peer ditolak gerbang");
                self.note_disconnected(peer);
            }
        }
    }

    /// Loop kendali mesh: listener, peristiwa koneksi, dan perintah daemon.
    #[allow(clippy::too_many_lines)]
    async fn run(
        mut self,
        listener: TcpListener,
        mut events_rx: mpsc::Receiver<PeerEvent>,
        mut commands_rx: mpsc::Receiver<P2pCommand>,
        token: CancellationToken,
    ) {
        loop {
            tokio::select! {
                biased;
                () = token.cancelled() => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, remote)) => {
                            if self.connected_count() >= self.config.max_peers {
                                drop(stream);
                                tracing::warn!(peer = %remote, "Tolak koneksi: batas peer tercapai");
                                continue;
                            }
                            let peer = remote.to_string();
                            let (link, rx) = mpsc::channel(PEER_CHANNEL_CAPACITY);
                            spawn_connection_task(
                                stream,
                                rx,
                                self.event_tx.clone(),
                                peer.clone(),
                                token.clone(),
                            );
                            let _ = self.event_tx.try_send(PeerEvent::Connected { peer, link });
                        }
                        Err(error) => {
                            tracing::warn!(%error, "Terima koneksi gagal");
                        }
                    }
                }
                event = events_rx.recv() => {
                    match event {
                        Some(PeerEvent::Connected { peer, link }) => {
                            self.register_peer(&peer, link);
                        }
                        Some(PeerEvent::Message { peer, message }) => {
                            self.handle_peer_message(&peer, &message);
                        }
                        Some(PeerEvent::Disconnected { peer }) => {
                            self.note_disconnected(&peer);
                        }
                        None => break,
                    }
                }
                command = commands_rx.recv() => {
                    match command {
                        Some(P2pCommand::Broadcast(message)) => self.broadcast(&message),
                        Some(P2pCommand::PeerCountQuery { reply }) => {
                            let _ = reply.send(self.connected_count());
                        }
                        None => break,
                    }
                }
            }
        }
        tracing::info!("Driver mesh P2P berhenti anggun");
    }
}

/// Task satu koneksi peer: frame masuk dikirim ke loop kendali, frame keluar
/// ditarik dari `link`. Tutup saat kawat putus atau token dibatalkan.
fn spawn_connection_task(
    stream: TcpStream,
    link_rx: mpsc::Receiver<NetworkMessage>,
    events: mpsc::Sender<PeerEvent>,
    peer: String,
    token: CancellationToken,
) {
    tokio::spawn(async move {
        let mut link_rx = link_rx;
        let mut connection = PeerConnection::new(stream);
        loop {
            tokio::select! {
                biased;
                () = token.cancelled() => break,
                outgoing = link_rx.recv() => {
                    match outgoing {
                        Some(message) => {
                            if connection.send_message(message).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                incoming = connection.read_message() => {
                    match incoming {
                        Ok(Some(message)) => {
                            if events
                                .try_send(PeerEvent::Message {
                                    peer: peer.clone(),
                                    message,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(None) | Err(_) => break,
                    }
                }
            }
        }
        let _ = events.try_send(PeerEvent::Disconnected { peer });
    });
}

/// Handle pemanggil driver P2P; murah dikloning.
#[derive(Debug, Clone)]
pub struct P2PHandle {
    commands: mpsc::Sender<P2pCommand>,
    token: CancellationToken,
    local_addr: SocketAddr,
}

impl P2PHandle {
    /// Siarkan satu pesan ke seluruh peer terhubung (gagal bila kanal penuh).
    pub fn broadcast(&self, message: NetworkMessage) {
        let _ = self.commands.try_send(P2pCommand::Broadcast(message));
    }

    /// Banyak peer yang sedang terhubung.
    pub async fn peer_count(&self) -> usize {
        let (reply, wait) = oneshot::channel();
        if self
            .commands
            .try_send(P2pCommand::PeerCountQuery { reply })
            .is_err()
        {
            return 0;
        }
        wait.await.map_or(0, |count| count)
    }

    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Hentikan seluruh koneksi dan loop mesh.
    pub fn shutdown(&self) {
        self.token.cancel();
    }

    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }
}

/// Hasil pasang mesh: handle pemanggil dan task loop kendali.
#[derive(Debug)]
pub struct P2PRuntime {
    /// Handle untuk daemon.
    pub handle: P2PHandle,
    /// Task loop kendali mesh.
    pub join: tokio::task::JoinHandle<()>,
}

impl P2PRuntime {
    /// Bind listener, jalankan redial, dan mulai loop kendali mesh.
    ///
    /// # Errors
    /// Mengembalikan galat bila alamat listen tidak dapat di-bind.
    pub async fn spawn(
        config: P2PConfig,
        actor: NodeHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(&config.listen_addr).await?;
        let local_addr = listener.local_addr()?;
        let listen_port = local_addr.port();
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let token = CancellationToken::new();

        for seed in &config.seeds {
            spawn_redial(seed.clone(), event_tx.clone(), token.clone());
        }

        let driver = MeshDriver::new(config, actor, event_tx, listen_port);
        let join = tokio::spawn(driver.run(listener, event_rx, command_rx, token.clone()));
        Ok(Self {
            handle: P2PHandle {
                commands: command_tx,
                token,
                local_addr,
            },
            join,
        })
    }
}

/// Task redial satu seed sampai berhasil atau token dibatalkan.
fn spawn_redial(seed: String, events: mpsc::Sender<PeerEvent>, token: CancellationToken) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(REDIAL_INTERVAL);
        loop {
            tokio::select! {
                biased;
                () = token.cancelled() => break,
                _ = interval.tick() => {
                    match TcpStream::connect(&seed).await {
                        Ok(stream) => {
                            let (link, rx) = mpsc::channel(PEER_CHANNEL_CAPACITY);
                            spawn_connection_task(
                                stream,
                                rx,
                                events.clone(),
                                seed.clone(),
                                token.clone(),
                            );
                            let _ = events.try_send(PeerEvent::Connected {
                                peer: seed.clone(),
                                link,
                            });
                            break;
                        }
                        Err(error) => {
                            tracing::debug!(peer = %seed, error = %error, "Redial seed gagal, coba lagi");
                        }
                    }
                }
            }
        }
    });
}
