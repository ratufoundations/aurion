use crate::{codec::AurionWireCodec, error::NetworkError, message::NetworkMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

pub struct PeerConnection {
    framed: Framed<TcpStream, AurionWireCodec>,
}

impl PeerConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            framed: Framed::new(stream, AurionWireCodec),
        }
    }

    /// Kirim satu pesan terenkode ke peer target
    pub async fn send_message(&mut self, msg: NetworkMessage) -> Result<(), NetworkError> {
        self.framed.send(msg).await
    }

    /// Baca pesan berikutnya dari stream jaringan
    pub async fn read_message(&mut self) -> Result<Option<NetworkMessage>, NetworkError> {
        match self.framed.next().await {
            Some(Ok(msg)) => Ok(Some(msg)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }
}
