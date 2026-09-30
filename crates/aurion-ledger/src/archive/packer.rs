use crate::archive::manifest::EpochManifest;
use crate::error::LedgerError;
use crate::schema::BLOCKS_TABLE;
use crate::Codec;
use aurion_core::Block;
use aurion_criptografi::Hasher;
use redb::Database;

/// Hasil paket arsip: buffer biner kanonikal + checksum BLAKE3.
#[derive(Debug)]
pub struct PackedArchive {
    pub blocks_bin: Vec<u8>,
    pub checksum: [u8; 32],
    pub manifest: EpochManifest,
}

/// Membaca rentang blok dari `BLOCKS_TABLE` dan menyusun buffer biner kanonikal.
///
/// Urutan blok terurut menaik dari `start_height` hingga `end_height` (AR1).
/// Checksum BLAKE3 dihitung atas seluruh buffer (AR2).
///
/// # Errors
/// Mengembalikan `LedgerError` jika blok tidak ditemukan atau data korup.
pub fn pack_range(
    db: &Database,
    epoch_index: u64,
    start_height: u64,
    end_height: u64,
    timestamp: u64,
) -> Result<PackedArchive, LedgerError> {
    let read_txn = db
        .begin_read()
        .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
    let table = read_txn
        .open_table(BLOCKS_TABLE)
        .map_err(|e| LedgerError::TableError(e.to_string()))?;

    let mut blocks_bin = Vec::new();
    let mut block_count: u32 = 0;
    let mut final_state_root = [0u8; 32];

    for height in start_height..=end_height {
        let block_bytes = table
            .get(height)
            .map_err(|e| LedgerError::StorageError(e.to_string()))?
            .ok_or(LedgerError::BlockNotFound(height))?;
        let block = Codec::decode_block(block_bytes.value())?;
        blocks_bin.extend_from_slice(&Codec::encode_block(&block));
        final_state_root = block.header.state_root;
        block_count += 1;
    }

    let checksum = Hasher::digest(&blocks_bin);

    let manifest = EpochManifest {
        epoch_index,
        start_height,
        end_height,
        block_count,
        final_state_root,
        timestamp,
    };

    Ok(PackedArchive {
        blocks_bin,
        checksum,
        manifest,
    })
}

/// Verifikasi integritas buffer blok terhadap checksum yang diharapkan (AR2).
///
/// # Errors
/// Mengembalikan `LedgerError::ChecksumMismatch` jika digest tidak cocok.
pub fn verify_checksum(blocks_bin: &[u8], expected: &[u8; 32]) -> Result<(), LedgerError> {
    let actual = Hasher::digest(blocks_bin);
    if actual != *expected {
        return Err(LedgerError::ChecksumMismatch);
    }
    Ok(())
}

/// Mendekode seluruh blok dari buffer biner kanonikal (AR1).
///
/// # Errors
/// Mengembalikan `LedgerError` jika buffer korup atau panjang tidak valid.
pub fn unpack_blocks(blocks_bin: &[u8]) -> Result<Vec<Block>, LedgerError> {
    let mut blocks = Vec::new();
    let mut offset = 0usize;

    while offset < blocks_bin.len() {
        if offset + Codec::HEADER_SIZE > blocks_bin.len() {
            return Err(LedgerError::CorruptedBlock(0));
        }
        let header_slice = &blocks_bin[offset..offset + Codec::HEADER_SIZE];
        let tx_count = u32::from_le_bytes([
            header_slice[72],
            header_slice[73],
            header_slice[74],
            header_slice[75],
        ]) as usize;
        let total_len = Codec::HEADER_SIZE + tx_count * Codec::TX_SIZE;
        if offset + total_len > blocks_bin.len() {
            return Err(LedgerError::CorruptedBlock(0));
        }
        let block = Codec::decode_block(&blocks_bin[offset..offset + total_len])?;
        blocks.push(block);
        offset += total_len;
    }

    Ok(blocks)
}
