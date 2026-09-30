//! Orkestrasi actor-pattern: perakitan seluruh subsistem Aurion menjadi satu
//! daemon simpul yang berjalan di atas `tokio`.
//!
//! Modul ini mengimplementasikan arsitektur kanal berbatas sesuai matriks
//! orkestrasi O0-O5. Prinsipnya dijelaskan pada dokumentasi crate.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use aurion_consensus::{RoundState, ValidatorSet, Vote, VoteType};
use aurion_core::{Block, BlockHeader, State, Transaction};
use aurion_criptografi::{Hash256, Keypair, PublicKeyBytes};
use aurion_execution::{
    Action, ActionBatch, ExecutionContext, ExecutionEngine, ExecutionPolicy, StoreKey,
};
use aurion_genesis::{GenesisBootstrap, GenesisSpec};
use aurion_guard::{BlacklistVerdict, GuardCouncil};
use aurion_ledger::LedgerStore;
use aurion_mempool::{Mempool, MempoolConfig};
use aurion_network::{AurionWireCodec, NetworkMessage};
use aurion_projection::{AddressIndex, ProjectionCursor, ReadSnapshot};

/// Satuan saldo (Quanta).
pub type Quanta = u64;

/// Prefix kunci saldo yang dipakai mesin eksekusi.
pub const PREFIX_BALANCE: &[u8] = b"bal:";

/// Nama namespace yang dimiliki `AccountKeeper` dalam mesin eksekusi.
const ACCOUNT_NAMESPACE: &str = "account";

/// Panjang digest namespace pada kunci terkualifikasi.
const NAMESPACE_DIGEST_LEN: usize = 32;

/// Kapasitas kanal ingress (Gateway → ChainNode).
pub const INGRESS_CHANNEL_CAPACITY: usize = 5_000;

/// Kapasitas kanal event (ChainNode → pengamat).
pub const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Kapasitas kanal gossip (jaringan ↔ ChainNode).
pub const GOSSIP_CHANNEL_CAPACITY: usize = 256;

/// Batas transaksi maksimum per blok secara bawaan.
pub const DEFAULT_MAX_TX_PER_BLOCK: u32 = 1_000;

/// Durasi blok bawaan dalam milidetik.
pub const DEFAULT_BLOCK_TIME_MS: u64 = 2_000;

/// Galat pada jalur ingress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressError {
    /// Kanal penuh: backpressure, pesan dibuang tanpa menumpukan memori.
    ChannelFull { capacity: usize },
    /// Node sedang berhenti.
    NodeStopping,
    /// Transaksi ditolak oleh validasi lokal.
    Rejected { reason: String },
}

impl std::fmt::Display for IngressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChannelFull { capacity } => {
                write!(f, "kanal ingress penuh (kapasitas {capacity})")
            }
            Self::NodeStopping => write!(f, "node sedang berhenti"),
            Self::Rejected { reason } => write!(f, "transaksi ditolak: {reason}"),
        }
    }
}

impl std::error::Error for IngressError {}

/// Kejadian yang dipublikasikan oleh actor rantai.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainEvent {
    /// Blok genesis berhasil dikomit saat bootstrapping.
    GenesisCommitted { height: u64, state_root: Hash256 },
    /// Transaksi diterima mempool.
    TransactionAccepted { height: u64, pending: usize },
    /// Blok dikomit ke ledger dan delta diteruskan ke read model.
    BlockCommitted {
        height: u64,
        state_root: Hash256,
        tx_count: u32,
        delta_entries: usize,
    },
    /// Jumlah suara BFT yang sudah masuk ke state konsensus.
    VotesObserved { height: u64, votes: usize },
    /// Hasil eksekusi putusan guard terhadap sebuah validator.
    GuardVerdictApplied {
        target: PublicKeyBytes,
        blacklisted: bool,
        /// `true` bila verdict menandai pelanggaran akut (double-signing) yang
        /// memerintahkan karantina ireversibel validator ke `Tombstoned`.
        tombstone_requested: bool,
    },
    /// Node berhenti anggun pada tinggi terakhir yang tercatat.
    ShutdownComplete { last_height: u64 },
}

/// Perintah yang diterima actor rantai.
#[derive(Debug)]
pub enum ChainCommand {
    /// Submit transaksi dari Gateway.
    Submit(Transaction),
    /// Jalankan satu ronde produksi blok.
    ProduceBlock,
    /// Kueri saldo read model.
    BalanceQuery {
        account: PublicKeyBytes,
        reply: oneshot::Sender<Option<Quanta>>,
    },
    /// Kueri tinggi blok terakhir yang tercatat di ledger.
    HeightQuery { reply: oneshot::Sender<u64> },
    /// Pesan jaringan dari actor swarm.
    Gossip(NetworkMessage),
    /// Putusan deferensi dari subsistem guard.
    GuardVerdict(BlacklistVerdict),
}

/// Konfigurasi perakitan simpul.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Identifier jaringan.
    pub chain_id: u64,
    /// Direktori basis data redb.
    pub data_dir: PathBuf,
    /// Akun treasury genesis.
    pub treasury_account: PublicKeyBytes,
    /// Validator awal yang didaftarkan.
    pub initial_validators: Vec<PublicKeyBytes>,
    /// Guard awal yang didaftarkan.
    pub initial_guards: Vec<PublicKeyBytes>,
    /// Kunci penanda tangan blok.
    pub block_proposer: PublicKeyBytes,
    /// Batas transaksi per blok.
    pub max_tx_per_block: u32,
    /// Kapasitas kanal ingress.
    pub ingress_capacity: usize,
    /// Jalankan bootstrapping genesis bila ledger masih kosong.
    pub bootstrap: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            chain_id: 1_001,
            data_dir: PathBuf::from("target/node-test-db"),
            treasury_account: [0xA1; 32],
            initial_validators: vec![[0xB1; 32]],
            // `GuardCouncil` mewajibkan kuorum penjaga minimum lima anggota.
            initial_guards: default_guards(),
            block_proposer: [0xB1; 32],
            max_tx_per_block: DEFAULT_MAX_TX_PER_BLOCK,
            ingress_capacity: INGRESS_CHANNEL_CAPACITY,
            bootstrap: true,
        }
    }
}

/// Roster penjaga bawaan yang memenuhi `MINIMUM_GUARD_QUORUM`.
#[must_use]
pub fn default_guards() -> Vec<PublicKeyBytes> {
    vec![[0xC1; 32], [0xC2; 32], [0xC3; 32], [0xC4; 32], [0xC5; 32]]
}

/// Actor rantai: pemilik eksklusif seluruh state simpul.
#[derive(Debug)]
pub struct ChainNode {
    config: NodeConfig,
    ledger: LedgerStore,
    state: State,
    mempool: Mempool,
    execution: ExecutionEngine,
    council: GuardCouncil,
    round: RoundState,
    cursor: ProjectionCursor,
    read_model: ReadSnapshot,
    address_index: AddressIndex,
    votes_seen: usize,
}

impl ChainNode {
    /// Rakit actor dari konfigurasi, membuka ledger dan menjalankan
    /// bootstrapping genesis bila diperlukan.
    ///
    /// # Errors
    /// Mengembalikan galat bila ledger gagal dibuka, spesifikasi genesis
    /// tidak valid, atau bootstrapping gagal.
    pub fn bootstrap(config: NodeConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = config.data_dir.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let ledger = LedgerStore::open(&config.data_dir)?;
        let needs_genesis = config.bootstrap && ledger.get_block_by_height(0)?.is_none();

        let spec = GenesisSpec::new(
            config.chain_id,
            "aurion-local",
            1_700_000_000,
            config.treasury_account,
            config.initial_validators.clone(),
            config.initial_guards.clone(),
        )?;

        let (state, genesis_root) = if needs_genesis {
            let block = GenesisBootstrap::initialize_ledger(&ledger, &spec)?;
            (State::new(), block.header.state_root)
        } else {
            (State::new(), [0u8; 32])
        };

        // Mutu state setelah genesis: akun treasury diinjeksi ulang agar read
        // model awal konsisten dengan basis data.
        let mut state = state;
        if needs_genesis {
            state.insert_account(
                config.treasury_account,
                aurion_core::Account::new(spec.total_supply_quanta()?, 0),
            );
        }

        let mut execution = ExecutionEngine::new(ExecutionPolicy::default(), [0u8; 32])?;
        if needs_genesis {
            execution.set_balance_genesis(&config.treasury_account, spec.total_supply_quanta()?)?;
        }

        let validator_set = ValidatorSet::new(config.initial_validators.clone());
        let height = ledger.get_latest_height()?;
        let round = RoundState::new(height + 1, 0, validator_set);
        let council =
            GuardCouncil::new(config.initial_guards.clone()).map_err(|e| e.to_string())?;

        let mut read_model = ReadSnapshot::new(height, genesis_root);
        // Read model harus sudah mencerminkan state genesis, bukan hanya setelah
        // blok pertama dikomit.
        for (account, entry) in state.accounts() {
            read_model.set_balance(*account, entry.balance);
        }

        Ok(Self {
            cursor: ProjectionCursor::new(),
            read_model,
            address_index: AddressIndex::new(),
            round,
            mempool: Mempool::new(MempoolConfig::default()),
            execution,
            council,
            state,
            ledger,
            votes_seen: 0,
            config,
        })
    }

    /// Tinggi blok terakhir yang tercatat di ledger.
    #[must_use]
    pub fn last_height(&self) -> u64 {
        self.ledger.get_latest_height().unwrap_or(0)
    }

    /// Jumlah transaksi yang menunggu di mempool.
    #[must_use]
    pub fn mempool_len(&self) -> usize {
        self.mempool.total_count()
    }

    /// Kueri saldo langsung dari read model.
    #[must_use]
    pub fn balance(&self, account: &PublicKeyBytes) -> Option<Quanta> {
        self.read_model.get_balance(account)
    }

    /// Jumlah entri pada indeks alamat.
    #[must_use]
    pub fn indexed_transactions(&self) -> usize {
        self.address_index
            .get_transaction_count(&self.config.treasury_account)
    }

    /// Jalankan loop actor sampai kanal ditutup atau token dibatalkan.
    pub async fn run(
        mut self,
        mut commands: mpsc::Receiver<ChainCommand>,
        mut gossip: mpsc::Receiver<NetworkMessage>,
        events: mpsc::Sender<ChainEvent>,
        token: CancellationToken,
    ) {
        loop {
            tokio::select! {
                biased;
                () = token.cancelled() => break,
                message = gossip.recv() => match message {
                    Some(message) => self.on_gossip(message, &events),
                    None => break,
                },
                command = commands.recv() => {
                    match command {
                        Some(ChainCommand::Submit(tx)) => self.on_submit(tx, &events),
                        Some(ChainCommand::ProduceBlock) => self.produce_block(&events),
                        Some(ChainCommand::BalanceQuery { account, reply }) => {
                            let _ = reply.send(self.balance(&account));
                        }
                        Some(ChainCommand::HeightQuery { reply }) => {
                            let _ = reply.send(self.last_height());
                        }
                        Some(ChainCommand::Gossip(message)) => self.on_gossip(message, &events),
                        Some(ChainCommand::GuardVerdict(verdict)) => self.on_verdict(&verdict, &events),
                        None => break,
                    }
                }
            }
        }

        // Penutupan terkoordinasi: laporkan tinggi terakhir lalu lepas
        // `ledger` sehingga redb menutup wal dan men-flush state.
        let last_height = self.last_height();
        let _ = events.try_send(ChainEvent::ShutdownComplete { last_height });
        tracing::info!(last_height, "Actor rantai berhenti anggun");
    }

    /// Terima transaksi dari Gateway.
    fn on_submit(&mut self, tx: Transaction, events: &mpsc::Sender<ChainEvent>) {
        let height = self.last_height();
        match self.mempool.insert(tx, &self.state) {
            Ok(()) => {
                let _ = events.try_send(ChainEvent::TransactionAccepted {
                    height,
                    pending: self.mempool.total_count(),
                });
            }
            Err(error) => {
                tracing::warn!(%error, "Transaksi ditolak mempool");
            }
        }
    }

    /// Terima pesan jaringan dan masukkan suara BFT ke state konsensus.
    fn on_gossip(&mut self, message: NetworkMessage, events: &mpsc::Sender<ChainEvent>) {
        if let NetworkMessage::Vote(vote) = message {
            let height = self.round_height();
            if self.round.add_vote(&vote).is_ok() {
                self.votes_seen += 1;
                let _ = events.try_send(ChainEvent::VotesObserved {
                    height,
                    votes: self.votes_seen,
                });
            }
        }
    }

    /// Tinggi ronde konsensus berjalan.
    fn round_height(&self) -> u64 {
        self.last_height() + 1
    }

    /// Terapkan putusan guard; daftar hitam langsung memblokir proposer.
    fn on_verdict(&mut self, verdict: &BlacklistVerdict, events: &mpsc::Sender<ChainEvent>) {
        let target = verdict.evidence.target_validator;
        let blacklisted = self.council.execute_blacklist(verdict).is_ok();
        if !blacklisted {
            tracing::warn!(?target, "Putusan guard ditolak, proposer tetap aktif");
        }
        // Pelanggaran akut (double-signing) memerintahkan karantina ireversibel
        // ke `Tombstoned`. Kunci konsensus pelaku dicatat di daftar cekal
        // permanen dewan; status `Tombstoned` pada mesin siklus hidup validator
        // diterapkan terpisah oleh orkestrator melalui `tombstone_validator`.
        let tombstone_requested = blacklisted && verdict.requires_tombstone();
        if tombstone_requested {
            match self.council.record_tombstone(&target) {
                Ok(()) => tracing::error!(
                    ?target,
                    "Guard memerintahkan tombstone: validator berstatus double-signing"
                ),
                Err(err) => {
                    tracing::warn!(?target, error = %err, "Tombstone sudah tercatat sebelumnya");
                }
            }
        }
        let _ = events.try_send(ChainEvent::GuardVerdictApplied {
            target,
            blacklisted,
            tombstone_requested,
        });
    }

    /// Produksi dan komit satu blok dari isi mempool.
    fn produce_block(&mut self, events: &mpsc::Sender<ChainEvent>) {
        if self.mempool.is_empty() {
            return;
        }

        let max_tx = usize::try_from(self.config.max_tx_per_block)
            .unwrap_or(DEFAULT_MAX_TX_PER_BLOCK as usize);
        let candidates = self
            .mempool
            .select_transactions_for_block(&self.state, max_tx);
        if candidates.is_empty() {
            return;
        }

        // Penjaga blacklist guard sebelum proposer distressed.
        if self.council.is_blacklisted(&self.config.block_proposer) {
            tracing::warn!("Proposer diblokir oleh daemon guard");
            return;
        }

        let current = self.last_height();
        let next_height = current + 1;
        let proposer = self.config.block_proposer;

        // (1) Otoritas ledger: `aurion_core::State` menentukan `state_root`
        //     yang sah untuk `LedgerStore::commit_block`.
        let mut shadow = self.state.clone();
        let mut committed_txs = Vec::with_capacity(candidates.len());
        for tx in candidates {
            if shadow
                .apply_transaction_with_proposer(&tx, proposer)
                .is_ok()
            {
                committed_txs.push(tx);
            }
        }
        if committed_txs.is_empty() {
            return;
        }

        // (2) Mesin eksekusi Capability-Keeper: membangkitkan `WriteSet` delta
        //     beserta akuntansi fuel untuk audit.
        let mut batch = ActionBatch::new();
        for tx in &committed_txs {
            batch.push(Action::Transfer {
                from: tx.sender,
                to: tx.recipient,
                amount: tx.amount,
            });
        }
        let ctx = ExecutionContext {
            block_height: next_height,
            timestamp: now_millis(),
        };
        let outcome = match self.execution.execute(&ctx, &batch) {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(%error, "Eksekusi batch ditolak");
                return;
            }
        };

        // (3) Rakit dan komit blok.
        let prev_hash = self
            .ledger
            .get_block_by_height(current)
            .ok()
            .flatten()
            .map_or([0u8; 32], |block| block.header.hash());

        let block = Block {
            header: BlockHeader {
                height: next_height,
                prev_hash,
                state_root: shadow.compute_state_root(),
                tx_count: committed_txs.len() as u32,
                timestamp: now_millis(),
                proposer,
            },
            transactions: committed_txs.clone(),
        };

        if let Err(error) = self.ledger.commit_block(&block, &mut self.state) {
            tracing::error!(%error, "Komit blok gagal");
            return;
        }
        self.mempool.prune_committed(&block, &self.state);

        // (4) Proyeksi read model dari state yang sudah dikomit — `aurion_core::
        //     State` adalah otoritas ledger, sehingga read model selalu
        //     mencerminkan blok yang benar-benar tersimpan.
        let write_set_entries = count_balance_entries(&outcome.delta);
        for (account, entry) in self.state.accounts() {
            self.read_model.set_balance(*account, entry.balance);
        }
        for tx in &committed_txs {
            self.address_index
                .add_transaction(tx, block.header.hash(), next_height);
        }
        self.cursor
            .advance(next_height, block.header.state_root)
            .ok();

        // Ronde BFT digeser ke tinggi berikutnya agar suara post-komit
        // tidak ditolak sebagai `HeightRoundMismatch`.
        self.round = RoundState::new(
            next_height + 1,
            0,
            ValidatorSet::new(self.config.initial_validators.clone()),
        );

        let _ = events.try_send(ChainEvent::BlockCommitted {
            height: next_height,
            state_root: block.header.state_root,
            tx_count: block.header.tx_count,
            delta_entries: write_set_entries,
        });
    }
}

/// Digest namespace yang dimiliki `AccountKeeper`.
fn account_namespace_digest() -> [u8; 32] {
    StoreKey::new(ACCOUNT_NAMESPACE).map_or([0u8; 32], |key| *key.namespace_digest())
}

/// Hitung entri saldo pada `WriteSet` mesin eksekusi.
///
/// `NamespaceStore` melekatkan setiap kunci pengguna dengan digest namespace,
/// sehingga bentuk kunci sebenarnya adalah
/// `namespace_digest(32) || "bal:" || pubkey(32)`. Hanya entri berbentuk
/// seperti itu yang mewakili mutasi saldo.
fn count_balance_entries(delta: &aurion_execution::WriteSet) -> usize {
    let digest = account_namespace_digest();
    delta
        .entries
        .keys()
        .filter(|key| {
            key.len() == NAMESPACE_DIGEST_LEN + PREFIX_BALANCE.len() + 32
                && key.starts_with(&digest)
                && key[NAMESPACE_DIGEST_LEN..NAMESPACE_DIGEST_LEN + PREFIX_BALANCE.len()]
                    == *PREFIX_BALANCE
        })
        .count()
}

///Milliseconds sejak epoch sebagai `u64`.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Handle pemanggil: satu-satunya jalan masuk ke actor rantai.
///
/// Hanya memuat pengirim kanal sehingga murah dikloning.
#[derive(Debug, Clone)]
pub struct NodeHandle {
    commands: mpsc::Sender<ChainCommand>,
    gossip_in: mpsc::Sender<NetworkMessage>,
    token: CancellationToken,
}

impl NodeHandle {
    /// Bangun handle sekaligus menjalankan task actor rantainya.
    ///
    /// Seluruh kanal yang dibuat berbatas: ingress mengikuti
    /// [`NodeConfig::ingress_capacity`], event mengikuti
    /// [`EVENT_CHANNEL_CAPACITY`], dan gossip mengikuti
    /// [`GOSSIP_CHANNEL_CAPACITY`].
    #[must_use]
    pub fn spawn(node: ChainNode) -> NodeRuntime {
        let ingress_capacity = node.config.ingress_capacity;
        let (command_tx, command_rx) = mpsc::channel(ingress_capacity);
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (gossip_tx, gossip_rx) = mpsc::channel(GOSSIP_CHANNEL_CAPACITY);
        let token = CancellationToken::new();

        let join = tokio::spawn(node.run(command_rx, gossip_rx, event_tx, token.clone()));

        NodeRuntime {
            handle: NodeHandle {
                commands: command_tx,
                gossip_in: gossip_tx,
                token: token.clone(),
            },
            events: event_rx,
            join,
            token,
        }
    }

    /// Kirim transaksi memakai `try_send` sehingga beban puncak menghasilkan
    /// backpressure, bukan antrean tanpa batas.
    ///
    /// # Errors
    /// Mengembalikan [`IngressError::ChannelFull`] saat kanal penuh dan
    /// [`IngressError::NodeStopping`] saat actor sudah berhenti.
    pub fn submit(&self, tx: Transaction) -> Result<(), IngressError> {
        self.commands
            .try_send(ChainCommand::Submit(tx))
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => IngressError::ChannelFull {
                    capacity: self.commands.max_capacity(),
                },
                mpsc::error::TrySendError::Closed(_) => IngressError::NodeStopping,
            })
    }

    /// Minta satu ronde produksi blok.
    ///
    /// # Errors
    /// Berperilaku sama seperti [`Self::submit`].
    pub fn produce_block(&self) -> Result<(), IngressError> {
        self.commands
            .try_send(ChainCommand::ProduceBlock)
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => IngressError::ChannelFull {
                    capacity: self.commands.max_capacity(),
                },
                mpsc::error::TrySendError::Closed(_) => IngressError::NodeStopping,
            })
    }

    /// Kueri saldo read model.
    ///
    /// # Errors
    /// Mengembalikan galat bila actor berhenti sebelum membalas.
    pub async fn balance(&self, account: PublicKeyBytes) -> Result<Option<Quanta>, IngressError> {
        let (reply, wait) = oneshot::channel();
        self.commands
            .try_send(ChainCommand::BalanceQuery { account, reply })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => IngressError::ChannelFull {
                    capacity: self.commands.max_capacity(),
                },
                mpsc::error::TrySendError::Closed(_) => IngressError::NodeStopping,
            })?;
        wait.await.map_err(|_| IngressError::NodeStopping)
    }

    /// Kueri tinggi blok terakhir yang tercatat di ledger.
    ///
    /// # Errors
    /// Mengembalikan galat bila actor berhenti sebelum membalas.
    pub async fn latest_height(&self) -> Result<u64, IngressError> {
        let (reply, wait) = oneshot::channel();
        self.commands
            .try_send(ChainCommand::HeightQuery { reply })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => IngressError::ChannelFull {
                    capacity: self.commands.max_capacity(),
                },
                mpsc::error::TrySendError::Closed(_) => IngressError::NodeStopping,
            })?;
        wait.await.map_err(|_| IngressError::NodeStopping)
    }

    /// Siarkan pesan jaringan ke actor rantai.
    ///
    /// # Errors
    /// Mengembalikan galat bila kanal gossip penuh atau tertutup.
    pub fn gossip(&self, message: NetworkMessage) -> Result<(), IngressError> {
        self.gossip_in.try_send(message).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => IngressError::ChannelFull {
                capacity: self.gossip_in.max_capacity(),
            },
            mpsc::error::TrySendError::Closed(_) => IngressError::NodeStopping,
        })
    }

    /// Kirim putusan guard ke actor rantai.
    ///
    /// # Errors
    /// Mengembalikan galat bila kanal penuh atau actor sudah berhenti.
    pub fn submit_verdict(&self, verdict: BlacklistVerdict) -> Result<(), IngressError> {
        self.commands
            .try_send(ChainCommand::GuardVerdict(verdict))
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => IngressError::ChannelFull {
                    capacity: self.commands.max_capacity(),
                },
                mpsc::error::TrySendError::Closed(_) => IngressError::NodeStopping,
            })
    }

    /// Pemicu graceful shutdown untuk seluruh actor.
    pub fn shutdown(&self) {
        self.token.cancel();
    }

    /// Token pembatalan yang dibagikan.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }
}

/// Hasil runtime: handle pemanggil, task actor, dan sisi terima kanal.
#[derive(Debug)]
pub struct NodeRuntime {
    /// Handle untuk pemanggil.
    pub handle: NodeHandle,
    /// Task actor rantai.
    pub join: tokio::task::JoinHandle<()>,
    /// Sisi terima kanal event.
    pub events: mpsc::Receiver<ChainEvent>,
    /// Token pembatalan bersama.
    pub token: CancellationToken,
}

impl NodeRuntime {
    /// Ambil kejadian berikutnya dari kanal event.
    pub async fn next_event(&mut self) -> Option<ChainEvent> {
        self.events.recv().await
    }

    /// Ambil kejadian berikutnya dengan batas waktu.
    ///
    /// # Returns
    /// `Some` bila kejadian tiba dalam batas waktu, `None` bila tidak.
    pub async fn next_event_timeout(
        &mut self,
        duration: std::time::Duration,
    ) -> Option<ChainEvent> {
        tokio::time::timeout(duration, self.events.recv())
            .await
            .ok()
            .flatten()
    }
}

/// Bentukkan `Vote` bertanda tangan yang sah untuk pengujian orkestrasi.
#[must_use]
pub fn sign_vote(
    keypair: &Keypair,
    block_hash: Hash256,
    height: u64,
    round: u32,
    vote_type: VoteType,
) -> Vote {
    let validator = keypair.public_key_bytes();
    let unsigned = Vote::new(validator, block_hash, height, round, vote_type, [0u8; 64]);
    let signature = keypair.sign(&unsigned.digest());
    Vote::new(validator, block_hash, height, round, vote_type, signature)
}

/// Bentukkan frame wire P2B dari sebuah pesan jaringan.
pub fn encode_gossip(message: &NetworkMessage) -> Result<Vec<u8>, aurion_network::NetworkError> {
    use tokio_util::codec::Encoder;
    let mut codec = AurionWireCodec;
    let mut dst = bytes::BytesMut::new();
    codec.encode(message.clone(), &mut dst)?;
    Ok(dst.to_vec())
}

/// Terapkan frame gossip dan kembalikan pesan yang berhasil didekode.
pub fn decode_gossip(frame: &[u8]) -> Result<Option<NetworkMessage>, aurion_network::NetworkError> {
    AurionWireCodec::decode_at_eof(frame)
}
