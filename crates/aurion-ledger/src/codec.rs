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
    // ACCOUNT CODEC (24 Bytes: Balance u128 + Nonce u64)
    // ==========================================
    #[must_use]
    pub fn encode_account(acc: &Account) -> [u8; 24] {
        let mut buf = [0u8; 24];
        buf[0..16].copy_from_slice(&acc.balance.to_le_bytes());
        buf[16..24].copy_from_slice(&acc.nonce.to_le_bytes());
        buf
    }

    #[must_use]
    pub fn decode_account(bytes: &[u8; 24]) -> Account {
        let mut balance_bytes = [0u8; 16];
        balance_bytes.copy_from_slice(&bytes[0..16]);
        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[16..24]);
        Account::new(
            u128::from_le_bytes(balance_bytes),
            u64::from_le_bytes(nonce_bytes),
        )
    }

    // ==========================================
    // TRANSACTION CODEC (168 Bytes Fixed)
    // ==========================================
    pub const TX_SIZE: usize = 168;

    #[must_use]
    pub fn encode_tx(tx: &Transaction) -> [u8; Self::TX_SIZE] {
        let mut buf = [0u8; Self::TX_SIZE];
        buf[0..8].copy_from_slice(&tx.nonce.to_le_bytes());
        buf[8..40].copy_from_slice(&tx.sender);
        buf[40..72].copy_from_slice(&tx.recipient);
        buf[72..88].copy_from_slice(&tx.amount.to_le_bytes());
        buf[88..104].copy_from_slice(&tx.fee.to_le_bytes());
        buf[104..168].copy_from_slice(&tx.signature);
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
        let nonce = u64::from_le_bytes(read_array(&slice[0..8])?);
        let sender = read_array(&slice[8..40])?;
        let recipient = read_array(&slice[40..72])?;
        let amount = u128::from_le_bytes(read_array(&slice[72..88])?);
        let fee = u128::from_le_bytes(read_array(&slice[88..104])?);
        let signature = read_array(&slice[104..168])?;

        Ok(Transaction::new(
            sender, recipient, amount, nonce, fee, signature,
        ))
    }

    // ==========================================
    // BLOCK CODEC (116 Bytes Header + N * 168 Bytes TX)
    // ==========================================
    pub const HEADER_SIZE: usize = 116;

    #[must_use]
    pub fn encode_block(block: &Block) -> Vec<u8> {
        let mut out =
            Vec::with_capacity(Self::HEADER_SIZE + (block.transactions.len() * Self::TX_SIZE));

        // Header
        out.extend_from_slice(&block.header.height.to_le_bytes());
        out.extend_from_slice(&block.header.prev_hash);
        out.extend_from_slice(&block.header.state_root);
        out.extend_from_slice(&block.header.tx_count.to_le_bytes());
        out.extend_from_slice(&block.header.timestamp.to_le_bytes());
        out.extend_from_slice(&block.header.proposer);

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
        let timestamp = u64::from_le_bytes(read_array(&bytes[76..84])?);
        let proposer = read_array(&bytes[84..116])?;

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
                timestamp,
                proposer,
            },
            transactions,
        })
    }
}
