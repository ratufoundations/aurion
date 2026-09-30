use crate::error::LedgerError;
use crate::schema::{BLOCKS_TABLE, BLOCK_INDEX_TABLE};
use crate::Codec;
use redb::{Database, ReadableTable};

/// Memangkas rentang blok dari `BLOCKS_TABLE` dan `BLOCK_INDEX_TABLE` secara atomik.
///
/// Pruner menolak operasi jika `end_height + safety_horizon >= latest_height`,
/// memastikan minimal K blok tetap tersedia di database (AR0).
/// Penghapusan dilakukan dalam satu write transaction redb (AR3).
/// `ACCOUNTS_TABLE` tidak disentuh (AR4).
///
/// # Errors
/// Mengembalikan `LedgerError` jika horizon tidak terpenuhi atau transaksi gagal.
///
/// # Panics
/// Fungsi ini tidak panic dalam kondisi normal.
pub fn prune_range(
    db: &Database,
    start_height: u64,
    end_height: u64,
    safety_horizon: u64,
) -> Result<(), LedgerError> {
    let latest_height = get_latest_height(db)?;

    if end_height
        .checked_add(safety_horizon)
        .is_none_or(|v| v >= latest_height)
    {
        return Err(LedgerError::PruneTooEarly {
            end_height,
            latest_height,
            safety_horizon,
        });
    }

    let write_txn = db
        .begin_write()
        .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
    {
        let mut blocks_table = write_txn
            .open_table(BLOCKS_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;
        let mut index_table = write_txn
            .open_table(BLOCK_INDEX_TABLE)
            .map_err(|e| LedgerError::TableError(e.to_string()))?;

        for height in start_height..=end_height {
            let block_bytes = blocks_table
                .get(height)
                .map_err(|e| LedgerError::StorageError(e.to_string()))?
                .and_then(|guard| {
                    Codec::decode_block(guard.value())
                        .map(|block| block.header.hash())
                        .ok()
                });
            if let Some(hash) = block_bytes {
                blocks_table
                    .remove(height)
                    .map_err(|e| LedgerError::StorageError(e.to_string()))?;
                index_table
                    .remove(&hash)
                    .map_err(|e| LedgerError::StorageError(e.to_string()))?;
            }
        }
    }

    write_txn
        .commit()
        .map_err(|e| LedgerError::CommitError(format!("Gagal commit prune: {e}")))?;

    Ok(())
}

fn get_latest_height(db: &Database) -> Result<u64, LedgerError> {
    let read_txn = db
        .begin_read()
        .map_err(|e| LedgerError::TransactionError(e.to_string()))?;
    let meta_table = read_txn
        .open_table(crate::schema::METADATA_TABLE)
        .map_err(|e| LedgerError::TableError(e.to_string()))?;
    let height = meta_table
        .get("latest_height")
        .map_err(|e| LedgerError::StorageError(e.to_string()))?
        .map_or(0, |value| {
            let bytes: [u8; 8] = value.value().try_into().unwrap_or([0u8; 8]);
            u64::from_le_bytes(bytes)
        });
    Ok(height)
}
