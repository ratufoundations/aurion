use redb::TableDefinition;

/// Tabel Akun: `PublicKey` [u8; 32] -> Balance u128 (16 bita) + Nonce u64 (8 bita) = [u8; 24]
pub const ACCOUNTS_TABLE: TableDefinition<&[u8; 32], &[u8; 24]> = TableDefinition::new("accounts");

/// Tabel Blok: Block Height (u64) -> Bita Kanonikal Blok
pub const BLOCKS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("blocks_by_height");

/// Indeks Hash: Block Hash [u8; 32] -> Block Height (u64)
pub const BLOCK_INDEX_TABLE: TableDefinition<&[u8; 32], u64> =
    TableDefinition::new("block_hash_index");

/// Metadata: String Key -> Value Bytes (misal: "`latest_height`", "`state_root`")
pub const METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("chain_metadata");

/// Partisi key/value untuk data modul dengan kunci biner namespaced.
pub const MODULE_KV_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("module_kv");
