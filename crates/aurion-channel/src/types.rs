use aurion_core::types::Quanta;
use aurion_criptografi::{verifikasi_tanda_tangan, Keypair, PublicKeyBytes};

use crate::error::ChannelError;

/// Identitas saluran 64-bit; diturunkan deterministik dari parameter open.
pub type ChannelId = [u8; 8];

/// Identitas akun: public key ed25519 32-byte (kanonikal ledger).
pub type AccountId = PublicKeyBytes;

/// Ukuran tiket wire off-chain 128 byte: `channel_id || nonce || transferred || sender_pubkey || signature`.
pub const TICKET_SIZE: usize = 128;

/// Preimage 64 byte yang ditandatangani: `channel_id || nonce || transferred || sender_pubkey`.
pub const TICKET_PREIMAGE_SIZE: usize = 64;

const STATUS_OPEN: u8 = 0;
const STATUS_CHALLENGING: u8 = 1;
const STATUS_SETTLED: u8 = 2;
const STATE_SCHEMA_VERSION: u8 = 1;

/// Fase lifecycle saluran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    Open,
    Challenging { expire_height: u64 },
    Settled,
}

impl ChannelStatus {
    #[must_use]
    pub fn discriminant(&self) -> u8 {
        match self {
            Self::Open => STATUS_OPEN,
            Self::Challenging { .. } => STATUS_CHALLENGING,
            Self::Settled => STATUS_SETTLED,
        }
    }

    /// `expire_height` bernilai `0` bila saluran tidak sedang dalam sanggah.
    #[must_use]
    pub fn expire_height(&self) -> u64 {
        match self {
            Self::Challenging { expire_height } => *expire_height,
            Self::Open | Self::Settled => 0,
        }
    }
}

/// State kanonikal saluran yang dipersist di namespace `channel`.
///
/// Serialisasi biner 130-byte (LE untuk seluruh integer):
/// `version(u8) || status(u8) || channel_id(8) || sender(32) || receiver(32)
///  || total_deposit(16) || settled_amount(16) || last_nonce(8)
///  || challenge_period(8) || expire_height(8)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelState {
    pub channel_id: ChannelId,
    pub sender: AccountId,
    pub receiver: AccountId,
    pub total_deposit: Quanta,
    pub settled_amount: Quanta,
    pub last_nonce: u64,
    pub challenge_period: u64,
    pub status: ChannelStatus,
}

impl ChannelState {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(130);
        buf.push(STATE_SCHEMA_VERSION);
        buf.push(self.status.discriminant());
        buf.extend_from_slice(&self.channel_id);
        buf.extend_from_slice(&self.sender);
        buf.extend_from_slice(&self.receiver);
        buf.extend_from_slice(&self.total_deposit.to_le_bytes());
        buf.extend_from_slice(&self.settled_amount.to_le_bytes());
        buf.extend_from_slice(&self.last_nonce.to_le_bytes());
        buf.extend_from_slice(&self.challenge_period.to_le_bytes());
        buf.extend_from_slice(&self.status.expire_height().to_le_bytes());
        buf
    }

    /// Mendekode record state; panjang dan versi selain kanonikal ditolak (fail-fast).
    ///
    /// # Errors
    /// Mengembalikan `ChannelError::SerializationFailure` bila lebar/skema tidak sah.
    pub fn decode(bytes: &[u8]) -> Result<Self, ChannelError> {
        const MIN_LEN: usize = 130;
        if bytes.len() != MIN_LEN {
            return Err(ChannelError::SerializationFailure(format!(
                "Panjang record channel harus {MIN_LEN} byte, menerima {}",
                bytes.len()
            )));
        }
        if bytes[0] != STATE_SCHEMA_VERSION {
            return Err(ChannelError::SerializationFailure(format!(
                "Versi skema state channel tidak dikenal: {}",
                bytes[0]
            )));
        }

        let channel_id: ChannelId = bytes[2..10].try_into().map_err(|_| {
            ChannelError::SerializationFailure("channel_id tidak 8 byte".to_string())
        })?;
        let mut sender = [0u8; 32];
        sender.copy_from_slice(&bytes[10..42]);
        let mut receiver = [0u8; 32];
        receiver.copy_from_slice(&bytes[42..74]);
        let total = u128::from_le_bytes(bytes[74..90].try_into().map_err(|_| {
            ChannelError::SerializationFailure("deposit tidak 16 byte".to_string())
        })?);
        let settled = u128::from_le_bytes(bytes[90..106].try_into().map_err(|_| {
            ChannelError::SerializationFailure("settled tidak 16 byte".to_string())
        })?);
        let nonce =
            u64::from_le_bytes(bytes[106..114].try_into().map_err(|_| {
                ChannelError::SerializationFailure("nonce tidak 8 byte".to_string())
            })?);
        let period =
            u64::from_le_bytes(bytes[114..122].try_into().map_err(|_| {
                ChannelError::SerializationFailure("period tidak 8 byte".to_string())
            })?);
        let expire =
            u64::from_le_bytes(bytes[122..130].try_into().map_err(|_| {
                ChannelError::SerializationFailure("expire tidak 8 byte".to_string())
            })?);

        let status = match bytes[1] {
            STATUS_OPEN => ChannelStatus::Open,
            STATUS_CHALLENGING => ChannelStatus::Challenging {
                expire_height: expire,
            },
            STATUS_SETTLED => ChannelStatus::Settled,
            other => {
                return Err(ChannelError::SerializationFailure(format!(
                    "Diskriminan status tidak dikenal: {other}"
                )));
            }
        };

        Ok(Self {
            channel_id,
            sender,
            receiver,
            total_deposit: total,
            settled_amount: settled,
            last_nonce: nonce,
            challenge_period: period,
            status,
        })
    }
}

/// Tiket off-chain `BalanceProof`: bukti saldo kumulatif transfer.
///
/// Signature dihitung atas preimage 64-byte:
/// `channel_id(8) || nonce(8 LE) || transferred(16 LE) || sender_pubkey(32)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceProof {
    pub channel_id: ChannelId,
    pub nonce: u64,
    pub transferred_amount: Quanta,
    pub signature: [u8; 64],
}

impl BalanceProof {
    #[must_use]
    pub fn new(channel_id: ChannelId, nonce: u64, transferred_amount: Quanta) -> Self {
        Self {
            channel_id,
            nonce,
            transferred_amount,
            signature: [0u8; 64],
        }
    }

    /// Membuat tiket bertanda tangan atas preimage 64-byte.
    #[must_use]
    pub fn signed(
        channel_id: ChannelId,
        nonce: u64,
        transferred_amount: Quanta,
        sender_pubkey: &PublicKeyBytes,
        keypair: &Keypair,
    ) -> Self {
        let proof = Self::new(channel_id, nonce, transferred_amount);
        let payload = proof.preimage(sender_pubkey);
        Self {
            signature: keypair.sign(&payload),
            ..proof
        }
    }

    /// Preimage 64-byte kanonikal yang ditandatangani.
    #[must_use]
    pub fn preimage(&self, sender_pubkey: &PublicKeyBytes) -> [u8; TICKET_PREIMAGE_SIZE] {
        let mut buf = [0u8; TICKET_PREIMAGE_SIZE];
        buf[0..8].copy_from_slice(&self.channel_id);
        buf[8..16].copy_from_slice(&self.nonce.to_le_bytes());
        buf[16..32].copy_from_slice(&self.transferred_amount.to_le_bytes());
        buf[32..64].copy_from_slice(sender_pubkey);
        buf
    }

    /// Encode tiket wire 128-byte lengkap (termasuk signature di 64 byte terakhir).
    #[must_use]
    pub fn encode_ticket(&self, sender_pubkey: &PublicKeyBytes) -> [u8; TICKET_SIZE] {
        let mut buf = [0u8; TICKET_SIZE];
        buf[..TICKET_PREIMAGE_SIZE].copy_from_slice(&self.preimage(sender_pubkey));
        buf[TICKET_PREIMAGE_SIZE..].copy_from_slice(&self.signature);
        buf
    }

    /// Mendekode tiket 128-byte menjadi bukti + public key sender.
    ///
    /// # Errors
    /// Mengembalikan `ChannelError::SerializationFailure` bila lebar ≠ 128.
    pub fn decode_ticket(bytes: &[u8]) -> Result<(Self, PublicKeyBytes), ChannelError> {
        if bytes.len() != TICKET_SIZE {
            return Err(ChannelError::SerializationFailure(format!(
                "Tiket BalanceProof harus {TICKET_SIZE} byte, menerima {}",
                bytes.len()
            )));
        }
        let mut channel_id = [0u8; 8];
        channel_id.copy_from_slice(&bytes[0..8]);
        let nonce = u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| {
            ChannelError::SerializationFailure("nonce tiket tidak 8 byte".to_string())
        })?);
        let transferred = u128::from_le_bytes(bytes[16..32].try_into().map_err(|_| {
            ChannelError::SerializationFailure("transferred tiket tidak 16 byte".to_string())
        })?);
        let mut sender_pubkey = [0u8; 32];
        sender_pubkey.copy_from_slice(&bytes[32..64]);
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&bytes[64..128]);
        Ok((
            Self {
                channel_id,
                nonce,
                transferred_amount: transferred,
                signature,
            },
            sender_pubkey,
        ))
    }

    /// Verifikasi Ed25519 signature atas preimage 64-byte (invarian CH3).
    ///
    /// # Errors
    /// Varian error lain selain ketidakcocokan signature dipropagasikan.
    pub fn verify(&self, sender_pubkey: &PublicKeyBytes) -> Result<bool, ChannelError> {
        let payload = self.preimage(sender_pubkey);
        Ok(verifikasi_tanda_tangan(sender_pubkey, &payload, &self.signature).is_ok())
    }
}
