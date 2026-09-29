//! Modul pemulihan kerusakan dan rekonstruksi dingin.
//!
//! Modul ini menerapkan pemulihan state proyeksi dari ledger
//! dan verifikasi determinisme (P4).
//!
//! Mekanisme inti `cold_rebuild` memutar ulang seluruh transaksi blok pada
//! rentang ketinggian yang diminta menggunakan mesin state `aurion_core::State`,
//! kemudian memverifikasi bahwa `state_root` hasil rekonstruksi identik dengan
//! `state_root` yang dikomit pada header blok. Dengan demikian, integritas
//! rekonstruksi dingin dibuktikan secara kriptografis, bukan sekadar dengan
//! membandingkan nilai skalar.

use aurion_core::{Block, State};
use aurion_ledger::LedgerStore;
use aurion_criptografi::Hash256;
use std::sync::Arc;

use crate::error::ProjectionError;
use crate::snapshot::ReadSnapshot;

/// Type alias untuk AccountId.
pub type AccountId = Hash256;

/// Type alias untuk Quanta.
pub type Quanta = u64;

/// Manajer pemulihan proyeksi.
#[derive(Debug)]
pub struct ProjectionRecovery {
    /// Referensi ke ledger store.
    ledger: Arc<LedgerStore>,
}

impl ProjectionRecovery {
    /// Buat manajer pemulihan baru.
    #[must_use]
    pub fn new(ledger: Arc<LedgerStore>) -> Self {
        Self { ledger }
    }

    /// Rekonstruksi state proyeksi dari nol dengan memutar ulang rantai blok.
    ///
    /// convene dengan alokasi genesis kosong; pada rantai yang mendanai akun
    /// di blok genesis, gunakan [`Self::cold_rebuild_with_genesis`].
    ///
    /// # Arguments
    /// * `from_height` - Ketinggian awal (inklusif)
    /// * `to_height` - Ketinggian akhir (eksklusif)
    ///
    /// # Returns
    /// `Ok(ReadSnapshot)` - Snapshot state proyeksi yang direkonstruksi.
    ///
    /// # Errors
    /// - `ProjectionError::BlockUnavailable` jika data blok tidak tersedia
    /// - `ProjectionError::StateRootMismatch` jika `state_root` rekonstruksi
    ///   tidak cocok dengan header blok yang dikomit
    /// - `ProjectionError::NonSequentialBlock` jika rentang tidak valid atau
    ///   blok yang dibaca tidak berurutan
    /// - `ProjectionError::Execution` jika transaksi gagal dieksekusi
    pub fn cold_rebuild(&self, from_height: u64, to_height: u64) -> Result<ReadSnapshot, ProjectionError> {
        self.cold_rebuild_with_genesis(from_height, to_height, State::new())
    }

    /// Rekonstruksi dingin dengan alokasi genesis yang eksplisit.
    ///
    /// Dana akun yang diberikan pada blok genesis tidak dapat diturunkan dari
    /// pemutaran ulang transaksi, karena genesis tidak memuat transaksi. Alokasi
    /// genesis karena itu harus diserahkan oleh pemanggil sebagai konstanta
    /// protokol, lalu seluruh rantai di atasnya diverifikasi ulang.
    ///
    /// # Arguments
    /// * `from_height` - Ketinggian awal (inklusif)
    /// * `to_height` - Ketinggian akhir (eksklusif)
    /// * `genesis` - State awal yang merepresentasikan alokasi blok genesis
    ///
    /// # Returns
    /// `Ok(ReadSnapshot)` - Snapshot state proyeksi yang direkonstruksi.
    ///
    /// # Errors
    /// - `ProjectionError::BlockUnavailable` jika data blok tidak tersedia
    /// - `ProjectionError::StateRootMismatch` jika `state_root` rekonstruksi
    ///   tidak cocok dengan header blok yang dikomit
    /// - `ProjectionError::NonSequentialBlock` jika rentang tidak valid atau
    ///   blok yang dibaca tidak berurutan
    /// - `ProjectionError::Execution` jika transaksi gagal dieksekusi
    pub fn cold_rebuild_with_genesis(
        &self,
        from_height: u64,
        to_height: u64,
        genesis: State,
    ) -> Result<ReadSnapshot, ProjectionError> {
        if to_height < from_height {
            return Err(ProjectionError::NonSequentialBlock {
                expected: from_height,
                got: to_height,
            });
        }

        // Rekonstruksi selalu dimulai dari alokasi genesis, bukan dari state
        // yang sudah ada: inilah "cold" rebuild.
        let mut final_state_root: Hash256 = genesis.compute_state_root();
        let mut state = genesis;
        let mut final_height = from_height;

        for height in from_height..to_height {
            let block = self.fetch_block(height)?;
            Self::apply_block_to_state(&mut state, &block, height)?;

            // Bukti integritas: state_root hasil rekonstruksi harus sama
            // dengan yang dikomit pada header blok.
            let computed_root = state.compute_state_root();
            if computed_root != block.header.state_root {
                return Err(ProjectionError::StateRootMismatch {
                    height,
                    expected: hex(&block.header.state_root),
                    got: hex(&computed_root),
                });
            }

            final_state_root = computed_root;
            final_height = height;
        }

        Ok(Self::snapshot_from_state(final_height, final_state_root, &state))
    }

    /// Ambil blok pada ketinggian tertentu dari ledger.
    fn fetch_block(&self, height: u64) -> Result<Block, ProjectionError> {
        self.ledger
            .get_block_by_height(height)
            .map_err(|_| ProjectionError::BlockUnavailable { height })?
            .ok_or(ProjectionError::BlockUnavailable { height })
    }

    /// Terapkan seluruh transaksi satu blok ke state rekonstruksi.
    ///
    /// Header blok divalidasi terhadap ketinggian yang diminta agar rebuild
    /// tidak diam-diam menerapkan blok dari posisi yang salah, dan jumlah
    /// transaksi harus konsisten dengan `tx_count` yang dikomit.
    fn apply_block_to_state(
        state: &mut State,
        block: &Block,
        expected_height: u64,
    ) -> Result<(), ProjectionError> {
        if block.header.height != expected_height {
            return Err(ProjectionError::NonSequentialBlock {
                expected: expected_height,
                got: block.header.height,
            });
        }

        if block.transactions.len() as u32 != block.header.tx_count {
            return Err(ProjectionError::MalformedDeltaValue {
                reason: format!(
                    "tx_count header tidak cocok dengan jumlah transaksi: header={} aktual={}",
                    block.header.tx_count,
                    block.transactions.len()
                ),
            });
        }

        for tx in &block.transactions {
            // Biaya harus dikreditkan ke proposer blok, sama persis dengan
            // semantik eksekusi pada `Block::execute` di aurion-core. Bila
            // replay memakai tujuan biaya yang berbeda, `state_root` hasil
            // rekonstruksi akan menyimpang dan rebuild gagal palsu.
            state.apply_transaction_with_proposer(tx, block.header.proposer)?;
        }

        Ok(())
    }

    /// Susun snapshot model baca dari state hasil rekonstruksi.
    fn snapshot_from_state(height: u64, state_root: Hash256, state: &State) -> ReadSnapshot {
        let mut snapshot = ReadSnapshot::new(height, state_root);
        for (account_id, account) in state.accounts() {
            snapshot.set_balance(*account_id, account.balance);
        }
        snapshot
    }

    /// Verifikasi determinisme mutlak: dua snapshot harus identik.
    ///
    /// Perbandingan mencakup ketinggian, `state_root`, dan seluruh peta saldo,
    /// sehingga divergensi sekecil apa pun pada state baca terdeteksi.
    ///
    /// Fungsi ini tidak menyentuh ledger, karena kedua snapshot sudah membawa
    /// seluruh state baca yang perlu dibandingkan.
    ///
    /// # Arguments
    /// * `snapshot_a` - Snapshot pertama
    /// * `snapshot_b` - Snapshot kedua
    ///
    /// # Returns
    /// `Ok(())` jika identik, `Err(ProjectionError::DigestMismatch)` jika tidak.
    ///
    /// # Errors
    /// - `ProjectionError::DigestMismatch` bila snapshot tidak identik
    pub fn verify_determinism(
        snapshot_a: &ReadSnapshot,
        snapshot_b: &ReadSnapshot,
    ) -> Result<(), ProjectionError> {
        if snapshot_a.height() != snapshot_b.height() {
            return Err(ProjectionError::DigestMismatch {
                expected: format!("height={}", snapshot_a.height()),
                got: format!("height={}", snapshot_b.height()),
            });
        }

        if snapshot_a.state_root() != snapshot_b.state_root() {
            return Err(ProjectionError::DigestMismatch {
                expected: format!("state_root={}", hex(snapshot_a.state_root())),
                got: format!("state_root={}", hex(snapshot_b.state_root())),
            });
        }

        if snapshot_a.balances() != snapshot_b.balances() {
            return Err(ProjectionError::DigestMismatch {
                expected: format!("saldo={}", balances_digest(snapshot_a.balances())),
                got: format!("saldo={}", balances_digest(snapshot_b.balances())),
            });
        }

        Ok(())
    }

    /// Rekonstruksi ulang state pada ketinggian tertentu dari nol.
    ///
    /// Dipakai sebagai sumber pembanding independen untuk
    /// [`Self::verify_determinism`].
    ///
    /// # Arguments
    /// * `height` - Ketinggian blok
    /// * `genesis` - Alokasi blok genesis
    ///
    /// # Returns
    /// `Ok(State)` hasil rekonstruksi, `Err(ProjectionError)` jika gagal.
    ///
    /// # Errors
    /// - `ProjectionError::BlockUnavailable` jika blok tidak tersedia
    /// - `ProjectionError::StateRootMismatch` bila root hasil rekonstruksi
    ///   tidak cocok dengan header
    /// - `ProjectionError::Execution` jika transaksi gagal dieksekusi
    pub fn get_state_at(&self, height: u64, genesis: State) -> Result<State, ProjectionError> {
        let mut state = genesis;

        for h in 0..=height {
            let block = self.fetch_block(h)?;
            Self::apply_block_to_state(&mut state, &block, h)?;

            let computed_root = state.compute_state_root();
            if computed_root != block.header.state_root {
                return Err(ProjectionError::StateRootMismatch {
                    height: h,
                    expected: hex(&block.header.state_root),
                    got: hex(&computed_root),
                });
            }
        }

        Ok(state)
    }
}

/// Format heksadesimal deterministik untuk pesan galat.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Digest deterministik atas peta saldo.
///
/// Kunci diurutkan sebelum di-hash sehingga digest tidak bergantung pada
/// urutan iterasi `HashMap`, dan pesan galat tetap reproducible.
fn balances_digest(balances: &std::collections::HashMap<AccountId, Quanta>) -> String {
    use std::fmt::Write as _;
    let mut entries: Vec<_> = balances.iter().collect();
    entries.sort_unstable_by_key(|(id, _)| **id);

    let mut out = String::new();
    for (id, balance) in entries {
        let _ = write!(out, "{}:{};", hex(id), balance);
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use aurion_core::Account;
    use aurion_core::Transaction;
    use aurion_criptografi::{Keypair, PublicKeyBytes};
    use std::collections::BTreeMap;

    /// Bangun transaksi yang ditandatangani sah.
    fn signed_tx(
        sender: &Keypair,
        recipient: PublicKeyBytes,
        amount: u64,
        nonce: u64,
        fee: u64,
    ) -> Transaction {
        let pk = sender.public_key_bytes();
        let unsigned = Transaction::new(pk, recipient, amount, nonce, fee, [0u8; 64]);
        let signature = sender.sign(&unsigned.digest());
        Transaction::new(pk, recipient, amount, nonce, fee, signature)
    }

    /// Bentuk kanonik `BTreeMap<[u8;32], u64>` agar bisa dibandingkan langsung.
    fn balances_of(state: &State) -> BTreeMap<AccountId, Quanta> {
        state
            .accounts()
            .iter()
            .map(|(id, acc)| (*id, acc.balance))
            .collect()
    }

    /// Bangun rantai deterministik berisi `count` blok yang mentransfer dana.
    ///
    /// Mengembalikan state akhir beserta `state_root` setiap tinggi blok,
    /// meniru apa yang akan dikomit header blok pada ledger.
    fn build_chain(count: u64) -> (State, Vec<Hash256>, PublicKeyBytes, PublicKeyBytes) {
        let alice = Keypair::from_bytes(&[1u8; 32]);
        let bob = Keypair::from_bytes(&[2u8; 32]);
        let alice_pk = alice.public_key_bytes();
        let bob_pk = bob.public_key_bytes();

        let mut state = State::new();
        state.insert_account(alice_pk, Account::new(1_000_000, 0));
        state.insert_account(bob_pk, Account::new(0, 0));

        let mut roots = Vec::new();
        for nonce in 0..count {
            let tx = signed_tx(&alice, bob_pk, 100, nonce, 1);
            state.apply_transaction(&tx).expect("transaksi harus valid");
            roots.push(state.compute_state_root());
        }

        (state, roots, alice_pk, bob_pk)
    }

    #[test]
    fn test_state_root_is_deterministic_across_runs() {
        let (state_a, roots_a, _, _) = build_chain(25);
        let (state_b, roots_b, _, _) = build_chain(25);

        assert_eq!(roots_a, roots_b, "state_root harus deterministik");
        assert_eq!(balances_of(&state_a), balances_of(&state_b));
        assert_eq!(state_a.compute_state_root(), state_b.compute_state_root());
    }

    #[test]
    fn test_replayed_balances_match_expected_transfers() {
        let (state, _, alice_pk, bob_pk) = build_chain(50);
        let balances = balances_of(&state);

        // 50 transfer x 100 = 5_000 berpindah dari Alice ke Bob.
        // Biaya 1 per transaksiDialokasikan ke fee sink protokol.
        assert_eq!(balances.get(&alice_pk), Some(&(1_000_000 - 5_000 - 50)));
        assert_eq!(balances.get(&bob_pk), Some(&5_000));
    }

    #[test]
    fn test_snapshot_from_state_carries_rebuilt_balances() {
        let (state, roots, alice_pk, bob_pk) = build_chain(10);
        let snapshot = ProjectionRecovery::snapshot_from_state(9, roots[9], &state);

        assert_eq!(snapshot.height(), 9);
        assert_eq!(*snapshot.state_root(), roots[9]);
        assert_eq!(snapshot.get_balance(&alice_pk), Some(1_000_000 - 1_000 - 10));
        assert_eq!(snapshot.get_balance(&bob_pk), Some(1_000));
    }

    #[test]
    fn test_verify_determinism_accepts_identical_rebuilds() {
        let (_, roots, alice_pk, _) = build_chain(20);

        let mut a = ReadSnapshot::new(19, roots[19]);
        a.set_balance(alice_pk, 999);
        let b = a.clone();

        assert!(ProjectionRecovery::verify_determinism(&a, &b).is_ok());
    }

    #[test]
    fn test_verify_determinism_rejects_divergent_balances() {
        let (_, roots, alice_pk, _) = build_chain(5);

        let mut a = ReadSnapshot::new(4, roots[4]);
        a.set_balance(alice_pk, 1_000);
        let mut b = ReadSnapshot::new(4, roots[4]);
        b.set_balance(alice_pk, 1_001);

        assert!(matches!(
            ProjectionRecovery::verify_determinism(&a, &b),
            Err(ProjectionError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn test_verify_determinism_rejects_height_divergence() {
        let a = ReadSnapshot::new(4, [1u8; 32]);
        let b = ReadSnapshot::new(5, [1u8; 32]);

        assert!(matches!(
            ProjectionRecovery::verify_determinism(&a, &b),
            Err(ProjectionError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn test_balances_digest_is_insertion_order_independent() {
        let mut first = std::collections::HashMap::new();
        first.insert([1u8; 32], 10_u64);
        first.insert([2u8; 32], 20_u64);

        let mut second = std::collections::HashMap::new();
        second.insert([2u8; 32], 20_u64);
        second.insert([1u8; 32], 10_u64);

        assert_eq!(balances_digest(&first), balances_digest(&second));
    }

    #[test]
    fn test_cold_rebuild_rejects_inverted_range() {
        let recovery = ProjectionRecovery::new(Arc::new(LedgerStore::open(
            std::env::temp_dir().join("aurion-projection-range-test"),
        ).expect("ledger sementara harus dapat dibuat")));

        assert!(matches!(
            recovery.cold_rebuild(10, 5),
            Err(ProjectionError::NonSequentialBlock { expected: 10, got: 5 })
        ));
    }

    #[test]
    fn test_hex_is_zero_padded_lowercase() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
        assert_eq!(hex(&[0u8; 32]).len(), 64);
    }
}
