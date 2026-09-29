use crate::error::LedgerError;
use aurion_core::{Account, Block, BlockHeader, Transaction};

fn read_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], LedgerError> {
    if bytes.len() != N {
        return Err(LedgerError::MalformedData);
    }
    let mut array = [0u8; N];
    array.copy_from_slice(bytes);
    Ok(array)
}

#[derive(Debug)]
pub struct Codec;

impl Codec {
    // ==========================================
    // ACCOUNT CODEC (16 Bytes)
    // ==========================================
    #[must_use]
    pub fn encode_account(acc: &Account) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&acc.balance.to_le_bytes());
        buf[8..16].copy_from_slice(&acc.nonce.to_le_bytes());
        buf
    }

    #[must_use]
    pub fn decode_account(bytes: &[u8; 16]) -> Account {
        let balance = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let nonce = u64::from_le_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);
        Account::new(balance, nonce)
    }

    // ==========================================
    // TRANSACTION CODEC (144 Bytes Fixed)
    // ==========================================
    pub const TX_SIZE: usize = 144;

    #[must_use]
    pub fn encode_tx(tx: &Transaction) -> [u8; Self::TX_SIZE] {
        let mut buf = [0u8; Self::TX_SIZE];
        buf[0..32].copy_from_slice(&tx.sender);
        buf[32..64].copy_from_slice(&tx.recipient);
        buf[64..72].copy_from_slice(&tx.amount.to_le_bytes());
        buf[72..80].copy_from_slice(&tx.nonce.to_le_bytes());
        buf[80..144].copy_from_slice(&tx.signature);
        buf
    }

    /// Mendekode satu transaksi dengan panjang payload yang kanonikal.
    ///
    /// # Errors
    /// Mengembalikan error bila panjang atau data payload transaksi tidak valid.
    pub fn decode_tx(slice: &[u8]) -> Result<Transaction, LedgerError> {
        if slice.len() != Self::TX_SIZE {
            return Err(LedgerError::InvalidTransactionLength {
                expected: Self::TX_SIZE,
                got: slice.len(),
            });
        }
        let sender = read_array(&slice[0..32])?;
        let recipient = read_array(&slice[32..64])?;
        let amount = u64::from_le_bytes(read_array(&slice[64..72])?);
        let nonce = u64::from_le_bytes(read_array(&slice[72..80])?);
        let signature = read_array(&slice[80..144])?;

        Ok(Transaction::new(
            sender, recipient, amount, nonce, signature,
        ))
    }

    // ==========================================
    // BLOCK CODEC (76 Bytes Header + N * 144 Bytes TX)
    // ==========================================
    pub const HEADER_SIZE: usize = 76;

    #[must_use]
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

    /// Mendekode blok dan seluruh transaksi yang terserialisasi.
    ///
    /// # Errors
    /// Mengembalikan error bila header, panjang blok, atau transaksi rusak.
    pub fn decode_block(bytes: &[u8]) -> Result<Block, LedgerError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(LedgerError::CorruptedBlock(0));
        }

        let height = u64::from_le_bytes(read_array(&bytes[0..8])?);
        let prev_hash = read_array(&bytes[8..40])?;
        let state_root = read_array(&bytes[40..72])?;
        let tx_count = u32::from_le_bytes(read_array(&bytes[72..76])?);

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
