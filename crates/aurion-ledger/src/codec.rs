use crate::error::LedgerError;
use aurion_core::{Account, Block, BlockHeader, Transaction};

pub struct Codec;

impl Codec {
    // ==========================================
    // ACCOUNT CODEC (16 Bytes)
    // ==========================================
    #[inline(always)]
    pub fn encode_account(acc: &Account) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&acc.balance.to_le_bytes());
        buf[8..16].copy_from_slice(&acc.nonce.to_le_bytes());
        buf
    }

    #[inline(always)]
    pub fn decode_account(bytes: &[u8; 16]) -> Account {
        let balance = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let nonce = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        Account::new(balance, nonce)
    }

    // ==========================================
    // TRANSACTION CODEC (144 Bytes Fixed)
    // ==========================================
    pub const TX_SIZE: usize = 144;

    pub fn encode_tx(tx: &Transaction) -> [u8; Self::TX_SIZE] {
        let mut buf = [0u8; Self::TX_SIZE];
        buf[0..32].copy_from_slice(&tx.sender);
        buf[32..64].copy_from_slice(&tx.recipient);
        buf[64..72].copy_from_slice(&tx.amount.to_le_bytes());
        buf[72..80].copy_from_slice(&tx.nonce.to_le_bytes());
        buf[80..144].copy_from_slice(&tx.signature);
        buf
    }

    pub fn decode_tx(slice: &[u8]) -> Result<Transaction, LedgerError> {
        if slice.len() != Self::TX_SIZE {
            return Err(LedgerError::InvalidTransactionLength {
                expected: Self::TX_SIZE,
                got: slice.len(),
            });
        }
        let sender = slice[0..32].try_into().unwrap();
        let recipient = slice[32..64].try_into().unwrap();
        let amount = u64::from_le_bytes(slice[64..72].try_into().unwrap());
        let nonce = u64::from_le_bytes(slice[72..80].try_into().unwrap());
        let signature = slice[80..144].try_into().unwrap();

        Ok(Transaction::new(
            sender, recipient, amount, nonce, signature,
        ))
    }

    // ==========================================
    // BLOCK CODEC (76 Bytes Header + N * 144 Bytes TX)
    // ==========================================
    pub const HEADER_SIZE: usize = 76;

    pub fn encode_block(block: &Block) -> Vec<u8> {
        let mut out =
            Vec::with_capacity(Self::HEADER_SIZE + (block.transactions.len() * Self::TX_SIZE));

        // Header
        out.extend_from_slice(&block.header.height.to_le_bytes());
        out.extend_from_slice(&block.header.prev_hash);
        out.extend_from_slice(&block.header.state_root);
        out.extend_from_slice(&block.header.tx_count.to_le_bytes());

        // Transactions
        for tx in &block.transactions {
            out.extend_from_slice(&Self::encode_tx(tx));
        }

        out
    }

    pub fn decode_block(bytes: &[u8]) -> Result<Block, LedgerError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(LedgerError::CorruptedBlock(0));
        }

        let height = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let prev_hash = bytes[8..40].try_into().unwrap();
        let state_root = bytes[40..72].try_into().unwrap();
        let tx_count = u32::from_le_bytes(bytes[72..76].try_into().unwrap());

        let expected_total_len = Self::HEADER_SIZE + (tx_count as usize * Self::TX_SIZE);
        if bytes.len() != expected_total_len {
            return Err(LedgerError::CorruptedBlock(height));
        }

        let mut transactions = Vec::with_capacity(tx_count as usize);
        let mut offset = Self::HEADER_SIZE;

        for _ in 0..tx_count {
            let tx_slice = &bytes[offset..offset + Self::TX_SIZE];
            transactions.push(Self::decode_tx(tx_slice)?);
            offset += Self::TX_SIZE;
        }

        Ok(Block {
            header: BlockHeader {
                height,
                prev_hash,
                state_root,
                tx_count,
            },
            transactions,
        })
    }
}
