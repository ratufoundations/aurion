#![forbid(unsafe_code)]

pub mod bootstrap;
pub mod error;
pub mod roles;
pub mod spec;

pub use bootstrap::GenesisBootstrap;
pub use error::GenesisError;
pub use roles::FederationTopology;
pub use spec::{GenesisSpec, QUANTA_PER_AUR, TOTAL_GENESIS_SUPPLY_AUR};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use aurion_criptografi::Keypair;
    use tempfile::NamedTempFile;

    #[test]
    fn test_genesis_initialization_with_66m_aur() {
        let temp_file = NamedTempFile::new().expect("test operation should succeed");
        let ledger = aurion_ledger::LedgerStore::open(temp_file.path())
            .expect("test operation should succeed");

        let treasury = Keypair::generate().public_key_bytes();
        let val1 = Keypair::generate().public_key_bytes();
        let val2 = Keypair::generate().public_key_bytes();
        let guard1 = Keypair::generate().public_key_bytes();

        let spec = GenesisSpec::new(
            1001,
            "aurion-mainnet",
            1_700_000_000,
            treasury,
            vec![val1, val2],
            vec![guard1],
        )
        .expect("test operation should succeed");

        // Verifikasi perhitungan Quanta
        let expected_quanta = 66_000_000 * 1_000_000u64;
        assert_eq!(
            spec.total_supply_quanta()
                .expect("test operation should succeed"),
            expected_quanta
        );

        // Inisialisasi Blok 0 ke Ledger
        let genesis_block = GenesisBootstrap::initialize_ledger(&ledger, &spec)
            .expect("test operation should succeed");

        assert_eq!(genesis_block.header.height, 0);
        assert_eq!(genesis_block.header.prev_hash, [0u8; 32]);
        assert_eq!(genesis_block.header.tx_count, 0);

        // Verifikasi saldo Treasury di disk redb
        let treasury_on_disk = ledger
            .get_account(&treasury)
            .expect("test operation should succeed")
            .expect("test operation should succeed");
        assert_eq!(treasury_on_disk.balance, expected_quanta);
        assert_eq!(treasury_on_disk.nonce, 0);

        // Pastikan simpul guard dan validator terdata
        assert!(spec.topology.is_validator(&val1));
        assert!(spec.topology.is_guard(&guard1));
        assert!(!spec.topology.is_guard(&val1));

        // Inisialisasi kedua kali harus ditolak
        let err = GenesisBootstrap::initialize_ledger(&ledger, &spec).unwrap_err();
        match err {
            GenesisError::AlreadyInitialized(_) => {}
            _ => panic!("Ekspektasi GenesisError::AlreadyInitialized"),
        }
    }
}
