#![forbid(unsafe_code)]

pub mod codec;
pub mod error;
pub mod message;
pub mod peer;

pub use codec::AurionWireCodec;
pub use error::NetworkError;
pub use message::{Handshake, NetworkMessage, AURION_NET_MAGIC, MAX_FRAME_SIZE};
pub use peer::PeerConnection;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use aurion_consensus::{Vote, VoteType};
    use aurion_core::Transaction;
    use aurion_criptografi::Keypair;
    use bytes::BytesMut;
    use tokio_util::codec::{Decoder, Encoder};

    #[test]
    fn test_codec_vote_roundtrip() {
        let keypair = Keypair::generate();
        let val_pk = keypair.public_key_bytes();
        let vote = Vote::new(val_pk, [0xAA; 32], 100, 2, VoteType::Precommit, [0xBB; 64]);
        let msg = NetworkMessage::Vote(vote.clone());
        let mut codec = AurionWireCodec;
        let mut buffer = BytesMut::new();
        codec
            .encode(msg, &mut buffer)
            .expect("test operation should succeed");
        let decoded = codec
            .decode(&mut buffer)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        match decoded {
            NetworkMessage::Vote(decoded_vote) => {
                assert_eq!(decoded_vote.validator, vote.validator);
                assert_eq!(decoded_vote.block_hash, vote.block_hash);
                assert_eq!(decoded_vote.height, 100);
                assert_eq!(decoded_vote.round, 2);
                assert_eq!(decoded_vote.vote_type, VoteType::Precommit);
            }
            _ => panic!("Tipe pesan hasil decode salah"),
        }
    }

    #[test]
    fn test_codec_transaction_roundtrip() {
        let alice = Keypair::generate();
        let bob = Keypair::generate();
        let tx = Transaction::new(
            alice.public_key_bytes(),
            bob.public_key_bytes(),
            50_000,
            1,
            2,
            [0x77; 64],
        );
        let msg = NetworkMessage::Transaction(tx.clone());
        let mut codec = AurionWireCodec;
        let mut buffer = BytesMut::new();
        codec
            .encode(msg, &mut buffer)
            .expect("test operation should succeed");
        let decoded = codec
            .decode(&mut buffer)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        match decoded {
            NetworkMessage::Transaction(decoded_tx) => {
                assert_eq!(decoded_tx.sender, tx.sender);
                assert_eq!(decoded_tx.recipient, tx.recipient);
                assert_eq!(decoded_tx.amount, 50_000);
                assert_eq!(decoded_tx.fee, 2);
                assert_eq!(decoded_tx.nonce, 1);
            }
            _ => panic!("Tipe pesan hasil decode salah"),
        }
    }

    #[tokio::test]
    async fn test_tcp_peer_loopback() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test operation should succeed");
        let addr = listener
            .local_addr()
            .expect("test operation should succeed");
        let server_task = tokio::spawn(async move {
            let (socket, _) = listener
                .accept()
                .await
                .expect("test operation should succeed");
            let mut peer = PeerConnection::new(socket);
            let msg = peer
                .read_message()
                .await
                .expect("test operation should succeed")
                .expect("test operation should succeed");
            if let NetworkMessage::Ping(nonce) = msg {
                peer.send_message(NetworkMessage::Pong(nonce))
                    .await
                    .expect("test operation should succeed");
            }
        });
        let client_socket = tokio::net::TcpStream::connect(addr)
            .await
            .expect("test operation should succeed");
        let mut client_peer = PeerConnection::new(client_socket);
        client_peer
            .send_message(NetworkMessage::Ping(42))
            .await
            .expect("test operation should succeed");
        let response = client_peer
            .read_message()
            .await
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        assert_eq!(response, NetworkMessage::Pong(42));
        server_task.await.expect("test operation should succeed");
    }
}
