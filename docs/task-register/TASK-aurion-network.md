# TASK: task-aurion-network
> **Target Modul:** `aurion-network` (`crates/aurion-network`)
> **Kategori:** Modul Jaringan Kawat P2P & Batas Komunikasi Validator (P2P Wire & Swarm Boundary)
> **Status:** ACTIVE / IN_PROGRESS
> **Otoritas:** `Agents.md`
> **Fondasi Jaringan:** `rust-libp2p` (Gossipsub, Noise, Identify, Yamux, TCP)
> **Tingkat Ketergantungan:** 2 (`aurion-core`, `aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-network` adalah garis pertahanan pertama simpul Aurion terhadap lalu lintas P2P terdistribusi. Untuk menghindari pembuatan ulang protokol gossip dan enkripsi kawat dari nol (*avoiding NIH syndrome*), Aurion mengadopsi standar industri **`rust-libp2p`**.

Namun, agar repositori tetap mematuhi aturan ketat `Agents.md` (Zero-Float, Zero-Unsafe, Zero-Unwrap), modul ini **wajib menerapkan Pola Isolasi Domain Wrapper (Facade Pattern)**:
* Tipe internal `libp2p` (seperti `Swarm`, `Multiaddr`, `Behaviour`, dan tipe-tipe bertipe *float* pada algoritma scoring) **dilarang keras bocor** ke modul bisnis lain (`aurion-consensus`, `aurion-mempool`, `aurion-core`).
* Komunikasi ke lapisan luar dibatasi hanya melalui kanal pesan asinkron (`tokio::sync::mpsc`) dan tipe domain murni Aurion.

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci wrapper `aurion-network` secara terisolasi** agar:
1. Memastikan serialisasi/deserialisasi biner pesan Aurion sebelum masuk ke payload libp2p bersifat 100% deterministik dan *lossless*.
2. Membatasi ukuran transmisi Gossipsub secara ketat untuk mencegah serangan *Out-Of-Memory* (OOM).
3. Menjamin otentikasi identitas validator via **Noise (Ed25519)** dan negosiasi protokol/`chain_id` via **Identify** berjalan deterministik.
4. Menolak payload gossip yang rusak atau terpotong tanpa memicu *panic* (`unwrap`).
5. Memastikan isolasi kegagalan peer di dalam `Swarm`: putusnya satu koneksi tidak mengganggu topologi *mesh* peer lainnya.
6. Mengunci seluruh antarmuka publik modul pada representasi integer murni `u64` Quanta/milidetik (*zero-float boundary*).
7. Menegakkan `#![forbid(unsafe_code)]` di seluruh baris kode internal crate `aurion-network`.

---

## 2. ARSITEKTUR ISOLASI DOMAIN WRAPPER (FACADE PATTERN)

Modul `aurion-network` membungkus seluruh kompleksitas `rust-libp2p` di balik antarmuka layanan `NetworkService`:

```text
AURION PROTOCOL CRATES
  [aurion-consensus]  [aurion-mempool]
    (BlockProposal, Vote)   (Tx Broadcast)
              \               /
               v             v
        [aurion-network]
          [NetworkFacade / NetworkService]
            - Zero-Float Boundary (All APIs expose u64)
            - Nonce & Signature Validation Guard
          [AurionWireCodec (Deterministik Binary Serializer)]
          [AurionBehaviour: Swarm Event Loop]
            |- libp2p::gossipsub (Topik: blocks, txs, votes)
            |- libp2p::noise (Enkripsi Ed25519)
            |- libp2p::identify (chain_id & protocol handshake)
            `- libp2p::yamux (Multiplexing stream)
                       |
                       v
            TCP Socket Stream ke Internet
```

---

## 3. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (N0-N5)

### [N0] Determinisme & Kesempurnaan Wire Codec (Lossless Wire Codec Round-Trip)
* Bangun pesan transaksi dan voting konsensus dengan tanda tangan kriptografi nyata.
* Enkapsulasi via `AurionWireCodec::encode`, decode via `AurionWireCodec::decode`.
* Hasil decode wajib identik byte-per-byte (`assert_eq!`); tanpa trailing bytes atau perubahan skalar integer.

### [N1] Pertahanan Batas Ukuran Pesan Gossip (Bounded Allocation & Gossipsub Max Transmit Defense)
* Konfigurasi `max_transmit_size = 4 * 1024 * 1024` byte.
* Publikasi/penerimaan melebihi batas ditolak `Err(NetworkError::FrameTooLarge { size, max_allowed })`.
* Parser tidak mengalokasikan memori melebihi batas aman saat paket raksasa disuntikkan.

### [N2] Integritas Handshake & Validasi Chain ID (Identify Protocol & Chain ID Guard)
* Dua simpul `chain_id = 1001` saling terhubung -> handshake disetujui, peer aktif.
* Simpul `chain_id = 9999` ke mainnet -> `Err(NetworkError::ChainIdMismatch { expected, got })`, koneksi diputus.
* Pesan transaksi dari peer belum handshake wajib diabaikan total (`UnauthenticatedMessage`).
* Versi wire usang -> `Err(NetworkError::IncompatibleProtocolVersion { expected, got })`.

### [N3] Ketahanan Payload Biner Cacat & Anti-Panic (Malformed Gossip Payload & Fuzzing Safety)
* 1.000 pesan biner acak ke topik `aurion/tx/v1` dan `aurion/consensus/v1`.
* `NetworkService` menangkap via `Result::Err(NetworkError::MalformedPayload)` + log peringatan.
* Loop `Swarm` tetap berjalan tanpa crash; magic salah -> `InvalidMagicBytes`; terpotong -> `UnexpectedEof`.

### [N4] Isolasi Kegagalan Peer & Pemutusan Anggun (Swarm Fault Isolation & Clean Teardown)
* Topologi 3 simpul virtual (A, B, C) dalam satu mesh Gossipsub.
* Matikan soket B paksa di tengah transmisi -> A dan C deteksi tanpa deadlock.
* A dan C tetap bertukar proposal blok dan voting normal; B -> `PeerStatus::Disconnected` dan dibersihkan.

### [N5] Batas Metrik & Waktu Nir-Pecahan (Zero-Float Wrapper Boundary & Integer Timeouts)
* Tidak ada `f32`/`f64` pada `struct`/`enum`/`trait` publik di `crates/aurion-network/src/`.
* Heartbeat Gossipsub, timeout koneksi, rata-rata latensi memakai aritmatika integer (`u64` ms/byte).
* Akumulasi byte dilindungi `checked_add`.

---

## 4. KONFIGURASI DEPENDENSI (MINIMAL FEATURE FLAGS)

`crates/aurion-network/Cargo.toml` dilarang memakai `features = ["full"]`. Minimal yang diizinkan (dipin ke versi cache lokal):

```toml
[dependencies]
libp2p = { version = "0.56", default-features = false, features = [
    "tokio",
    "tcp",
    "noise",
    "yamux",
    "gossipsub",
    "identify",
    "macros",
] }
tokio = { version = "1.38", features = ["sync", "rt", "net", "time"] }
bytes = "1.6"
thiserror = "1.0"
tracing = "0.1"
aurion-core = { path = "../aurion-core" }
aurion-criptografi = { path = "../aurion-criptografi" }

[dev-dependencies]
tokio = { version = "1.38", features = ["macros", "rt-multi-thread"] }
```

Catatan implementasi: `tokio-util`, `futures-util`, `aurion-consensus`, `aurion-ledger` hanya dipertahankan bila dibutuhkan modul yang masih dipakai; target akhir facade hanya bergantung pada `aurion-core` + `aurion-criptografi` + libp2p minimal.

---

## 5. ARSITEKTUR TEST SUITE & LOKASI FILE

* **Target Modul:** `crates/aurion-network/`
* **Lokasi Suite Uji:** `crates/aurion-network/tests/network_invariants.rs`
* **Metode Pengujian:** in-memory / local loopback TCP socket (`127.0.0.1:0`) dengan multi-node mock swarm untuk N0-N5 deterministik.

---

## 6. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

1. Berkas spesifikasi tersimpan di `docs/task-register/TASK-aurion-network.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji N0-N5 di `crates/aurion-network/tests/network_invariants.rs` lulus: `cargo test -p aurion-network --test network_invariants`.
3. Unit test inline tetap lulus: `cargo test -p aurion-network`.
4. Lolos audit statis: `cargo clippy -p aurion-network --all-targets --all-features -- -D warnings` dan `python3 tools/aurion_guard.py check`.
5. Tidak ada kebocoran tipe *float* atau tipe mentah `libp2p` pada antarmuka publik crate.
6. Status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.
