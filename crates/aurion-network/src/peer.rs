use crate::{codec::AurionWireCodec, error::NetworkError, message::NetworkMessage};
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

    /// Kirim satu pesan terenkode ke peer target
    pub async fn send_message(&mut self, msg: NetworkMessage) -> Result<(), NetworkError> {
        self.framed.send(msg).await
    }

    /// Baca pesan berikutnya dari stream jaringan
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
