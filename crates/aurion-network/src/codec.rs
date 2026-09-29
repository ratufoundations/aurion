use crate::{
    error::NetworkError,
    message::{Handshake, NetworkMessage, AURION_NET_MAGIC, MAX_FRAME_SIZE},
};
use aurion_ledger::Codec as LedgerCodec;
use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

fn read_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], NetworkError> {
    if bytes.len() != N {
        return Err(NetworkError::MalformedPayload);
    }
    let mut array = [0u8; N];
    array.copy_from_slice(bytes);
    Ok(array)
}

#[derive(Debug)]
pub struct AurionWireCodec;

impl AurionWireCodec {
    pub const HEADER_LEN: usize = 8;
}

impl Encoder<NetworkMessage> for AurionWireCodec {
    type Error = NetworkError;

    fn encode(&mut self, item: NetworkMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let type_id = item.message_type_id();
        let payload = match item {
            NetworkMessage::Handshake(hs) => {
                let mut buf = Vec::with_capacity(42);
                buf.extend_from_slice(&hs.node_id);
                buf.extend_from_slice(&hs.chain_id.to_le_bytes());
                buf.extend_from_slice(&hs.listen_port.to_le_bytes());
                buf
            }
            NetworkMessage::Transaction(tx) => LedgerCodec::encode_tx(&tx).to_vec(),
            NetworkMessage::Block(block) => LedgerCodec::encode_block(&block),
            NetworkMessage::Vote(vote) => NetworkMessage::encode_vote(&vote).to_vec(),
            NetworkMessage::Ping(nonce) => nonce.to_le_bytes().to_vec(),
            NetworkMessage::Pong(nonce) => nonce.to_le_bytes().to_vec(),
        };
        let frame_payload_len = 1 + payload.len();
        if frame_payload_len > MAX_FRAME_SIZE {
            return Err(NetworkError::FrameTooLarge {
                size: frame_payload_len,
                limit: MAX_FRAME_SIZE,
            });
        }
        dst.reserve(Self::HEADER_LEN + frame_payload_len);
        dst.put_u32(AURION_NET_MAGIC);
        dst.put_u32(frame_payload_len as u32);
        dst.put_u8(type_id);
        dst.put_slice(&payload);
        Ok(())
    }
}

impl Decoder for AurionWireCodec {
    type Item = NetworkMessage;
    type Error = NetworkError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < Self::HEADER_LEN {
            return Ok(None);
        }
        let magic = u32::from_be_bytes(read_array(&src[0..4])?);
        if magic != AURION_NET_MAGIC {
            return Err(NetworkError::InvalidMagic {
                expected: AURION_NET_MAGIC,
                got: magic,
            });
        }
        let payload_len = u32::from_be_bytes(read_array(&src[4..8])?) as usize;
        if payload_len == 0 {
            return Err(NetworkError::MalformedPayload);
        }
        if payload_len > MAX_FRAME_SIZE {
            return Err(NetworkError::FrameTooLarge {
                size: payload_len,
                limit: MAX_FRAME_SIZE,
            });
        }
        if src.len() < Self::HEADER_LEN + payload_len {
            src.reserve(Self::HEADER_LEN + payload_len - src.len());
            return Ok(None);
        }
        tracing::trace!(
            bytes = Self::HEADER_LEN + payload_len,
            "Frame wire TCP masuk"
        );
        src.advance(Self::HEADER_LEN);
        let type_id = src.get_u8();
        let body_len = payload_len - 1;
        let body = src.split_to(body_len);
        match type_id {
            0x01 => {
                if body.len() != 42 {
                    return Err(NetworkError::MalformedPayload);
                }
                let node_id = read_array(&body[0..32])?;
                let chain_id = u64::from_le_bytes(read_array(&body[32..40])?);
                let listen_port = u16::from_le_bytes(read_array(&body[40..42])?);
                Ok(Some(NetworkMessage::Handshake(Handshake {
                    node_id,
                    chain_id,
                    listen_port,
                })))
            }
            0x02 => {
                let tx = LedgerCodec::decode_tx(&body)?;
                Ok(Some(NetworkMessage::Transaction(tx)))
            }
            0x03 => {
                let block = LedgerCodec::decode_block(&body)?;
                Ok(Some(NetworkMessage::Block(block)))
            }
            0x04 => {
                let vote = NetworkMessage::decode_vote(&body)?;
                Ok(Some(NetworkMessage::Vote(vote)))
            }
            0x05 => {
                if body.len() != 8 {
                    return Err(NetworkError::MalformedPayload);
                }
                let nonce = u64::from_le_bytes(read_array(&body[..])?);
                Ok(Some(NetworkMessage::Ping(nonce)))
            }
            0x06 => {
                if body.len() != 8 {
                    return Err(NetworkError::MalformedPayload);
                }
                let nonce = u64::from_le_bytes(read_array(&body[..])?);
                Ok(Some(NetworkMessage::Pong(nonce)))
            }
            unknown => Err(NetworkError::UnknownMessageType(unknown)),
        }
    }
}
