# TASK-aurion-archive: Ledger State Pruning & Epoch ZIP Archival Engine

## 1. Metadata
* **Status:** IN_PROGRESS
* **Crate:** `crates/aurion-ledger`
* **Dependensi:** `aurion-core`, `aurion-criptografi`, `redb`, `zip`, `serde`, `serde_json`
* **Batasan Protokol:** `#![forbid(unsafe_code)]`, zero-float murni, I/O atomik, deterministik byte-for-byte.

---

## 2. Arsitektur Pemangkasan & Pembekuan
1. **Pemisahan Hot State vs. Cold Archive:**
   * **Hot State (`ACCOUNTS_TABLE`):** Menyimpan saldo mutakhir `[u8; 24]` secara permanen di VPS (operasi Read/Update).
   * **Prunable Log (`BLOCKS_TABLE`):** Blok-blok yang telah melampaui batas aman (*Safety Horizon*, misal $K = 1.000$ blok) dipangkas dari `redb` setelah dikompresi ke arsip ZIP.
2. **Struktur Berkas Arsip Kanonikal (`aurion-epoch-{index}.zip`):**
   * `manifest.json`: Berisi `epoch_index`, `start_height`, `end_height`, `block_count`, `final_state_root` (hex), dan `timestamp`.
   * `blocks.bin`: Sekuens biner kanonikal dari `BlockHeader` (116B) dan transaksi (N × 168B).
   * `checksum.blake3`: Digest BLAKE3 derivasi atas `blocks.bin` untuk validasi integritas instan.
3. **Recovery & Fast-Replay:**
   * Node auditor atau pengarsip dapat memverifikasi isi `blocks.bin` terhadap manifest tanpa perlu menyalakan seluruh node jaringan.

---

## 3. Matriks Invarian Pengujian (AR0 – AR5)

| ID | Invarian | Batasan & Verifikasi |
|---|---|---|
| **AR0** | **Safety Horizon Retention** | Pemangkas menolak memotong blok jika tinggi rantai belum melewati batas `horizon` (blok aktif minimal $K$ selalu tersisa di database). |
| **AR1** | **Deterministic Binary Packing** | Urutan byte pada `blocks.bin` tersusun konsisten: blok terurut menaik (`start_height..=end_height`), tanpa padding acak. |
| **AR2** | **Checksum Verification Integrity** | Hash BLAKE3 yang dihitung atas `blocks.bin` cocok 100% dengan `checksum.blake3` pada arsip ZIP. Modifikasi 1 bit pada isi ZIP memicu kegagalan validasi. |
| **AR3** | **Atomic Prune & Rollback** | Penghapusan blok dari `redb` dieksekusi dalam transaksi write atomik. Jika proses kompresi atau penulisan disk gagal, database `redb` tidak boleh terpotong sebagian. |
| **AR4** | **Hot State Invariance** | Pemangkasan `BLOCKS_TABLE` tidak mengubah isi, saldo, maupun nonce di `ACCOUNTS_TABLE`. Verifikasi saldo sebelum dan sesudah pruning identik byte-per-byte. |
| **AR5** | **Zero-Float Manifest Conformance** | `manifest.json` hanya memuat integer diskrit (`u64`, `string hex`) dan menolak nilai float dalam serialisasi serde. |

---

## 4. CATATAN IMPLEMENTASI

1. **Kompresi ZIP:** Menggunakan crate `zip` dengan `CompressionMethod::Deflated` (flate2). Arsip ditulis ke file sementara lalu di-rename atomik untuk mencegah korupsi partial.
2. **Checksum BLAKE3:** Dihitung via `aurion_criptografi::Hasher::digest` atas seluruh `blocks.bin` sebelum penulisan ZIP.
3. **Prune atomik:** Penghapusan blok dilakukan dalam satu `redb` write transaction. Jika write transaction gagal, seluruh perubahan di-rollback oleh redb.
4. **Safety horizon:** Pruner menolak operasi jika `end_height + safety_horizon >= latest_height`, memastikan minimal K blok tetap tersedia di database.
5. **Manifest zero-float:** Struct `EpochManifest` hanya menggunakan tipe integer (`u64`, `u32`) dan byte array. Deserialisasi serde_json secara alami menolak nilai float untuk field integer.

---

## 5. Verifikasi
- `cargo check --workspace --all-targets`
- `cargo test -p aurion-ledger`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all --check`
- `python3 tools/aurion_guard.py check` (exit 0)