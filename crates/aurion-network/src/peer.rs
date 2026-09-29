use crate::{
    codec::AurionWireCodec,
    error::NetworkError,
    message::{HandshakeStatus, NetworkMessage},
};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[derive(Debug)]
pub struct PeerConnection {
    framed: Framed<TcpStream, AurionWireCodec>,
    peer_addr: Option<SocketAddr>,
}

impl PeerConnection {
    pub fn new(stream: TcpStream) -> Self {
        let peer_addr = stream.peer_addr().ok();
        Self {
            framed: Framed::new(stream, AurionWireCodec),
            peer_addr,
        }
    }

    /// Kirim satu pesan terenkode ke peer target.
    ///
    /// # Errors
    /// Mengembalikan error bila encoding atau penulisan ke peer gagal.
    pub async fn send_message(&mut self, msg: NetworkMessage) -> Result<(), NetworkError> {
        self.framed.send(msg).await
    }

    /// Baca pesan berikutnya dari stream jaringan.
    ///
    /// # Errors
    /// Mengembalikan error bila frame tidak valid atau stream gagal dibaca.
    pub async fn read_message(&mut self) -> Result<Option<NetworkMessage>, NetworkError> {
        match self.framed.next().await {
            Some(Ok(msg)) => {
                if matches!(msg, NetworkMessage::Handshake(_)) {
                    tracing::info!(peer = ?self.peer_addr, "Frame handshake peer P2P diterima");
                }
                Ok(Some(msg))
            }
            Some(Err(error)) => {
                tracing::warn!(peer = ?self.peer_addr, error = %error, "Koneksi peer P2P mengalami galat");
                Err(error)
            }
            None => {
                tracing::warn!(peer = ?self.peer_addr, "Koneksi peer terputus");
                Ok(None)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerStatus {
    Connected,
    Disconnected,
}

/// Gerbang autentikasi per koneksi: pesan pertama wajib handshake yang valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedGate {
    expected_chain_id: u64,
    authenticated: bool,
}

impl AuthenticatedGate {
    #[must_use]
    pub const fn new(expected_chain_id: u64) -> Self {
        Self {
            expected_chain_id,
            authenticated: false,
        }
    }

    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        self.authenticated
    }

    /// Terima satu pesan masuk; handshake valid membuka gerbang.
    ///
    /// # Errors
    /// Mengembalikan `ChainIdMismatch` / `IncompatibleProtocolVersion` bila
    /// handshake tidak valid, atau `UnauthenticatedMessage` bila pesan
    /// non-handshake tiba sebelum autentikasi.
    pub fn admit(&mut self, msg: &NetworkMessage) -> Result<Option<HandshakeStatus>, NetworkError> {
        match msg {
            NetworkMessage::Handshake(handshake) => {
                let status = handshake.validate(self.expected_chain_id)?;
                self.authenticated = true;
                tracing::info!(chain_id = handshake.chain_id, "Handshake peer tervalidasi");
                Ok(Some(status))
            }
            _ => {
                if self.authenticated {
                    Ok(None)
                } else {
                    Err(NetworkError::UnauthenticatedMessage)
                }
            }
        }
    }
}
