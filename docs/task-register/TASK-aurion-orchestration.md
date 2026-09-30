# TASK: task-aurion-orchestration
> **Target Modul:** `aurion-node` (`apps/aurion-node`)  
> **Kategori:** Integrasi Sistem, Orkestrasi Daemon, & Manajemen Siklus Hidup Node (System Integration & Daemon Orchestration)  
> **Status:** COMPLETED / VERIFIED  
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper)  
> **Tingkat Ketergantungan:** 4 (Seluruh Modul Inti: `aurion-core`, `aurion-criptografi`, `aurion-ledger`, `aurion-mempool`, `aurion-consensus`, `aurion-account`, `aurion-validator`, `aurion-execution`, `aurion-guard`, `aurion-gateway`, `aurion-projection`, `aurion-network`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-node` (aplikasi biner utama) adalah titik kulminasi dari seluruh pustaka modular Aurion. Tugas modul ini adalah merakit 11 subsistem independen menjadi satu entitas simpul (*sovereign node daemon*) yang berjalan secara asinkron di atas *runtime* `tokio`.

Karena setiap modul telah lulus uji invarian secara terisolasi, risiko terbesar pada tahap ini bukanlah bug logika bisnis, melainkan **kegagalan konkurensi (deadlocks), kebocoran memori (channel congestion), *race conditions* saat inisialisasi, dan korupsi data akibat penutupan paksa (*abrupt shutdown*)**.

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci arsitektur orkestrasi `aurion-node`** agar:
1. Menjamin *bootstrapping* simpul dari *genesis state* (inisialisasi ledger, treasury, dan registri validator) berjalan secara deterministik murni tanpa kondisi balapan (*race condition*).
2. Memastikan seluruh subsistem berkomunikasi hanya melalui saluran pesan asinkron berbatas (*bounded channels*), menghilangkan risiko *lock contention* yang memblokir utas konsensus.
3. Menyatukan aliran data secara end-to-end: Transaksi (Gateway) $\to$ Mempool $\to$ Konsensus $\to$ Eksekusi (Guard/Validator) $\to$ Ledger $\to$ Proyeksi.
4. Memvalidasi bahwa sinyal interupsi sistem operasi (`SIGINT`/`SIGTERM`) dikelola secara anggun (*graceful shutdown*), menjamin *state* di-flush ke disk tanpa meninggalkan *dirty writes*.
5. Mengunci biner akhir sistem pada kepatuhan strict protokol: `#![forbid(unsafe_code)]` dan zero-float di seluruh lapisan integrasi aplikasi.

---

## 2. ARSITEKTUR ORKESTRASI & WIRING (ACTOR PATTERN)

Aplikasi akan merutekan lalu lintas antar-subsistem menggunakan `tokio::sync::mpsc`:

```text
                                [ Klien / External ]
                                         │
                                         ▼
   ┌─────────────────────────────────────────────────────────────────────────┐
   │ [Task 1: Ingress Gateway] (aurion-gateway)                              │
   └──────────┬───────────────────────────────────────────────────▲──────────┘
              │ (tx_ingress_chan)                                 │ (query)
              ▼                                                   │
   ┌─────────────────────────┐                            ┌───────┴──────────┐
   │ [Task 2: Mempool]       │                            │ [Task 6: Read    │
   │ (aurion-mempool)        │                            │  Projection]     │
   └──────────┬──────────────┘                            │ (aurion-         │
              │ (tx_batch_chan)                           │  projection)     │
              ▼                                           └───────▲──────────┘
   ┌─────────────────────────┐                                    │
   │ [Task 3: Consensus Loop]│◄──┐ (p2p_gossip_in_chan)           │ (delta)
   │ (aurion-consensus)      │   │                                │
   └──────────┬──────────────┘   │                        ┌───────┴──────────┐
              │ (block_commit)   │                        │ [Task 5: Ledger &│
              ▼                  │                        │  Execution Pipe] │
   ┌─────────────────────────┐   │                        │ - aurion-exec    │
   │ [Task 4: P2P Swarm]     ├───┘                        │ - aurion-guard   │
   │ (aurion-network)        │◄── (p2p_gossip_out_chan) ──│ - aurion-ledger  │
   └─────────────────────────┘                            └──────────────────┘

```

---

## 3. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (O0 – O5)

Suite integrasi level aplikasi (`orchestration_invariants.rs`) wajib menguji 6 dimensi berikut:

### [O0] Bootstrap & Invarian Genesis Deterministik (Bootstrap & Genesis Invariance)

* **Deskripsi:** Saat simpul dijalankan dari basis data kosong (`latest_height == 0`), simpul wajib memuat file `genesis.json`, menginisialisasi saldo `Treasury`, dan mendaftarkan `ActiveValidator` pertama sebelum daemon mulai mendengarkan port jaringan.
* **Kriteria Uji:**
  * Penghapusan folder data dan inisialisasi ulang wajib menghasilkan hash `state_root` genesis yang identik byte-per-byte pada setiap iterasi.
  * *Bootstrapping* gagal secara instan (*panic-free, fail-fast* dengan `Error`) jika file konfigurasi cacat, port P2P/RPC sudah terpakai (EADDRINUSE), atau kunci Ed25519 simpul tidak valid.

### [O1] Isolasi Saluran Pesan & Pencegahan Deadlock (Actor Task Channel Wiring & Anti-Deadlock)

* **Deskripsi:** Antar-subsistem dilarang menggunakan *shared mutability* lintas utas (seperti `Arc<RwLock<T>>` yang mengunci I/O). Semua transfer state di *hot path* (konsensus & mempool) wajib menggunakan `tokio::sync::mpsc`.
* **Kriteria Uji:**
  * Pengiriman 10.000 transaksi secara *burst* ke Gateway wajib mengisi mempool tanpa memblokir atau menunda detak jantung (*heartbeat*) konsensus.
  * Kanal wajib berjenis `bounded` (misal: kapasitas 5.000 pesan). Jika kanal penuh akibat beban berlebih, pengirim (*Gateway*) wajib menjatuhkan pesan secara anggun (*backpressure* dengan galat `RateLimit/Overload`) tanpa meruntuhkan sistem (*No OOM*).

### [O2] Aliran Pipa Eksekusi Blok End-to-End (Block Execution Pipeline Integrity)

* **Deskripsi:** Memastikan rantai pasokan dari proposal konsensus hingga proyeksi terbaca dapat berjalan sebagai satu kesatuan logika yang utuh.
* **Kriteria Uji:**
  * Suntikkan transaksi via Gateway API internal (O2 Mock).
  * Tunggu *event* komitmen blok dari saluran konsensus.
  * Blok dikonsumsi oleh utas *Pipeline*, diproses oleh `ExecutionEngine`, diperiksa lewat `GuardEngine`, dikomit ke `LedgerStore`, dan memancarkan `WriteSet` delta ke `ProjectionEngine`.
  * Verifikasi kueri ke proyeksi di akhir siklus memvalidasi bahwa mutasi state benar-benar tercermin di hasil bacaan *Read Model*.

### [O3] Sinkronisasi Swarm P2P & Loop Konsensus (Swarm & Consensus Synchronization)

* **Deskripsi:** Jembatan antara perutean pesan jaringan **in-daemon**
  (`apps/aurion-node/src/p2p.rs` — `MeshDriver`/`P2PRuntime` TCP, *bukan*
  `rust-libp2p`) dan pengambil keputusan `aurion-consensus`.
* **Kriteria Uji:**
  * *Loopback mesh* dalam satu *runtime test* memori: `o3_three_node_loopback_mesh_propagates_votes` (3 simpul semu) dan `o3_four_node_mesh_reaches_bft_quorum` (4 simpul menuju kuorum BFT **3-of-4**, `2f+1 = 3`).
  * Suara BFT (*vote*) dari Simpul A yang disiarkan lewat wire format wajib ditangkap dan didekode oleh Simpul B, lalu dimasukkan ke dalam mesin state konsensus Simpul B.
  * Pesan *gossip* palsu yang digagalkan oleh `AurionWireCodec` di level jaringan tidak boleh membebani atau mencapai saluran baca konsensus.

### [O4] Penutupan Anggun & Jaminan Flush State (Graceful Shutdown & State Flush Guarantee)

* **Deskripsi:** Sinyal `SIGINT` (Ctrl+C) atau `SIGTERM` wajib memicu penutupan terkoordinasi (bukan `std::process::exit(1)` mentah).
* **Kriteria Uji:**
  * Saat `CancellationToken` dipicu, utas Gateway langsung menolak request baru (HTTP 503).
  * Loop Konsensus tidak akan memulai ronde proposal baru, melainkan menyelesaikan komitmen blok yang sedang berjalan.
  * Basis data `redb` ditutup dengan sukses tanpa korupsi log *Write-Ahead*.
  * Instance simpul uji yang diinterupsi wajib dapat dijalankan kembali dari status terakhirnya tanpa terdeteksi adanya korupsi pada basis data.

### [O5] Audit Zero-Float Sistem Utuh (Zero-Float System-Wide Audit)

* **Deskripsi:** Menegakkan jaminan nir-pecahan hingga ke lapisan konfigurasi aplikasi (CLI/TOML) dan pelaporan log.
* **Kriteria Uji:**
  * Pembacaan parameter `config.toml` (seperti `timeout_ms`, `gas_limit`, `max_peers`) divalidasi murni sebagai tipe integer/string.
  * Audit pemindaian leksikal di dalam direktori `apps/aurion-node/src/` tidak boleh menemukan satupun representasi `f32` atau `f64`.

---

## 4. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian terpusat di `apps/aurion-node/tests/`:

* **`orchestration_invariants.rs`:** Menguji matriks **O0 – O5** menggunakan topologi *multi-node in-memory cluster* (tanpa benar-benar membuka socket publik).
* **`multiprocess_harness.rs`:** Harness **4 proses nyata** atas TCP sungguhan
  (`NODE_COUNT = 4`, ambang `2f+1 = 3`, `TIMEOUT = 90s`); `m0` membuktikan
  klaster 4 proses ber-BFT mandiri dan siap menerima transaksi.
* **`HeightQuery`:** `ChainCommand::HeightQuery { reply: oneshot::Sender<u64> }`
  (`apps/aurion-node/src/node.rs`) — kueri asinkron tinggi ledger terkini yang
  digunakan O4 dan harness lintas-proses untuk menunggu kemajuan.
* Dependensi: Memfaatkan fungsionalitas `tokio::test` untuk menciptakan beberapa *Actor Task* yang mensimulasikan lingkungan eksekusi asinkron riil.

---

## 5. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-orchestration` dinyatakan selesai apabila:

1. Berkas spesifikasi ini terdaftar di `docs/task-register/TASK-aurion-orchestration.md` dan diindeks pada `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh *wiring* antar-kanal diimplementasikan di `apps/aurion-node/src/node.rs` tanpa memicu *deadlock*.
3. Seluruh matriks uji O0–O5 lulus via:
```bash
cargo test -p aurion-node --test orchestration_invariants
```

4. Aplikasi berhasil melewati *bootstrapping* bersih dan dapat dikompilasi sebagai biner eksekusi akhir:
```bash
cargo build --release -p aurion-node
```

5. Lolos audit kepatuhan statis absolut:
```bash
cargo clippy -p aurion-node --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check
```

6. Status `COMPLETED` didokumentasikan di `logs/WORK_LOG.md`.

---

## 6. CATATAN IMPLEMENTASI (PENYIMPANGAN DARI DESKRIPSI AWAL)

Beberapa nama komponen pada bagian 2–3 tidak ada persis seperti tertulis pada
kode yang ada. Implementasi memakai API nyata berikut, tanpa membuat mock
subsistem:

1. **`GuardEngine` tidak ada.** Komponen keamanan yang nyata adalah
   `aurion_guard::GuardCouncil` dengan `execute_blacklist`/`execute_pardon`.
   Syaratnya persetujuan bulat 100% anggota guard dan
   `MINIMUM_GUARD_QUORUM = 5`, sehingga konfigurasi bawaan menyediakan lima
   penjaga (`default_guards()`).
2. **`ProjectionEngine` adalah unit struct kosong.** Proyeksi nyata dijalankan
   lewat `ProjectionCursor`, `AddressIndex`, `ReadSnapshot`, dan
   `SnapshotManager`. Read model diproyeksi dari `aurion_core::State` yang
   sudah dikomit, sehingga hasil bacaan selalu mencerminkan blok tersimpan.
3. **`ExecutionEngine` tidak kompatibel dengan `State`.** State-nya key/value
   terpisah dan tidak dapat langsung menjadi sumber `LedgerStore::commit_block`.
   Karena itu `aurion_core::State` menjadi otoritas ledger, sedangkan
   `ExecutionEngine` berfungsi sebagai gerbang eksekusi Capability-Keeper
   (WriteSet delta dan akuntansi fuel).
4. **Bentuk kunci saldo pada WriteSet adalah terkualifikasi**, yaitu
   `namespace_digest(32) || "bal:" || pubkey(32)`, bukan `bal:` mentah,
   karena `NamespaceStore` melekatkan digest namespace pada setiap kunci.
5. **`AurionGateway` tidak dapat diuji pada level kanal.** Constructor-nya
   memerlukan sesi Zenoh langsung dan menyimpan `Arc<Mutex<Mempool>>`
   sendiri, sehingga ia tidak cocok dengan pola kanal mpsc. Kontrak
   backpressure diuji pada kanal ingress bertingkat `NodeHandle`, sementara
   logarithmic throughput Gateway tidak diuji di sini.
6. **`gas_limit` tidak ada pada skema konfigurasi.** Parameter nyata yang
   dipakai adalah `max_tx_per_block`, `quorum_threshold_percent`,
   `query_timeout_ms`, dan `flush_interval_ms`; batas fuel sebagai gantinya berasal dari
   `ExecutionPolicy` mesin eksekusi.
7. **Kanal gossip dibaca langsung oleh actor rantai.** Message `Gossip` tidak
   lagi diteruskan lewat task jembatan tersendiri, karena kanal gossip
   langsung diseleksi di dalam `ChainNode::run`.
8. **Tidak ada `genesis.json`.** Bootstrapping memakai
   `GenesisBootstrap::initialize_ledger` dengan `GenesisSpec` terprogram,
   sehingga `state_root` genesis deterministik antar-basis data.
9. **Driver P2P bersifat in-daemon.** Kanal gossip tidak memakai `rust-libp2p`;
   `apps/aurion-node/src/p2p.rs` menyediakan `MeshDriver`/`P2PRuntime` TCP
   dengan wire frame `AurionWireCodec` (dari `aurion-network`). O3 kini
   mencakup kuorum 4-simpul 3-of-4 (`o3_four_node_mesh_reaches_bft_quorum`).
10. **Harness multiproses nyata.** `multiprocess_harness.rs` menjalankan 4 biner
    terpisah atas TCP sungguhan (bukan mesh memori): `NODE_COUNT = 4`,
    `2f+1 = 3`, `TIMEOUT = 90s`; `m0_four_process_cluster_reaches_bft_quorum_on_real_tcp`
    menunggu tanda `AURION_QUORUM_READY` sampai semua proses mencapai kuorum.
11. **`HeightQuery`** adalah `ChainCommand::HeightQuery { reply: oneshot::Sender<u64> }`
    di `node.rs`; dipakai untuk sinkronisasi tinggi lintas aktor/proses (kueri
    baca non-blokir, nilai `u64`).
