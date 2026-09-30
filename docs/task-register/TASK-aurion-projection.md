# TASK: task-aurion-projection
> **Target Modul:** `aurion-projection` (`crates/aurion-projection`)  
> **Kategori:** Mesin Proyeksi Model Baca CQRS, Indeksasi & Kueri Cepat (CQRS Read Model & Indexing Engine)  
> **Status:** COMPLETED  
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper)  
> **Tingkat Ketergantungan:** 3 (`aurion-core`, `aurion-ledger`, `aurion-execution`, `aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-projection` adalah mesin pengelola model baca (*Read Model*) dalam pemisahan arsitektur *Command Query Responsibility Segregation* (CQRS) protokol Aurion. Modul ini bertanggung jawab mengonsumsi aliran delta mutasi state (*WriteSet* dari `aurion-execution` E5) dan blok terfinalisasi dari `aurion-ledger` untuk membangun indeks kueri cepat yang dioptimalkan untuk pembacaan eksternal (kueri gateway GW1).

Data konsensus dan eksekusi disimpan dalam format penyimpanan pohon pasangan kunci-nilai (*key-value storage*) yang dirancang untuk integritas komitmen kriptografis, bukan untuk efisiensi kueri histori pengguna. Jika aplikasi penjelajah blok (*explorer*) atau dompet kueri memindai seluruh rantai secara berulang, performa simpul akan runtuh. `aurion-projection` menyediakan indeks proyeksi sekunder (riwayat transaksi per alamat, status proposal, statistik kinerja validator, dan snapshot saldo) secara asinkron tanpa pernah memegang *lock* penulisan pada mesin konsensus maupun *ledger*.

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-projection` secara terisolasi** agar:
1. Memastikan konsumsi aliran delta mutasi (*Delta Stream Ingestion*) bersifat 100% idempoten dan terikat pada kursor ketinggian blok (*monotonic projection cursor*).
2. Membangun dan menguji indeks transaksi berbasis alamat (*Address-Centric Indexing*) dengan paginasi berbatas (*bounded pagination*) tanpa memindai seluruh riwayat blok.
3. Menjamin konsistensi titik waktu (*point-in-time snapshot consistency*) pada model baca serta pelacakan ketertinggalan proyeksi (*projection lag tracking*) secara deterministik.
4. Menerapkan pembersihan riwayat usang (*Safe Pruning & Retention Window*) pada data event/tanda terima transaksi tanpa merusak integritas saldo aktif akun.
5. Menjamin ketahanan terhadap kegagalan mendadak (*Crash Recovery*) dan membuktikan bahwa rekonstruksi indeks dari nol (*Cold Rebuild*) menghasilkan state proyeksi yang identik byte-per-byte.
6. Menghitung seluruh rasio efisiensi cache (*Cache Hit Rate BPS*), latensi kueri, ukuran indeks, dan batas jendela waktu murni menggunakan integer `u64` (*zero-float*).
7. Menegakkan disiplin kode: `#![forbid(unsafe_code)]`, zero-float, penolakan `.unwrap()` di jalur produksi, serta penanganan error bertipe kuat melalui `thiserror`.

---

## 2. ARSITEKTUR CQRS READ MODEL & PIPELINE PROYEKSI

```text
       [Blok Terfinalisasi (L0)]          [WriteSet State Delta (E5)]
                   |                                   |
                   +─────────────────+─────────────────+
                                     |
                                     v
       +────────────────────────────────────────────────────────+
       |               AURION PROJECTION ENGINE                 |
       |                                                        |
       |   [P0] Ingestion Pipeline & Idempotent Cursor Guard    |
       |        - Validasi height == cursor + 1                 |
       |        - Penolakan duplikasi delta / replay            |
       +─────────────────────────────+──────────────────────────+
                                     |
                  +──────────────────+──────────────────+
                  v                                     v
       +─────────────────────+               +─────────────────────+
       | [P1] Address Index  |               | [P2] Read View      |
       | - Sent/Received Txs |               | - Account Balances  |
       | - Bounded Pagination|               | - Proposal State    |
       +──────────+──────────+               +──────────+──────────+
                  |                                     |
                  +──────────────────+──────────────────+
                                     |
                                     v
       +────────────────────────────────────────────────────────+
       |   [P3] Pruning & Bounded Retention Worker              |
       |        - Pembersihan event di luar MAX_RETENTION       |
       |   [P5] Zero-Float Performance Meter                    |
       |        - Tracking Lag: ledger_height - proj_height     |
       +─────────────────────────────+──────────────────────────+
                                     |
                                     v
                   [Kueri Cepat Gateway / Explorer]
                   (CQRS Read Path: Zero Lock Contention)
```

---

## 3. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (P0 - P5)

Suite pengujian modul wajib menguji 6 dimensi invarian proyeksi baca berikut:

### [P0] Konsumsi Delta & Idempotensi Proyeksi (Delta Stream Ingestion & Idempotent Invariance)

* **Deskripsi:** Pemasukan *WriteSet* atau blok ke dalam mesin proyeksi wajib terurut secara monotonik sesuai ketinggian blok ($H_{next} = H_{current} + 1$). Pemutaran ulang (*re-applying*) delta yang sama dilarang menduplikasi data histori atau memicu inkonsistensi saldo.
* **Kriteria Uji:**
  * Konsumsi sekuensial blok $H=1, 2, 3$ wajib memperbarui kursor proyeksi (`projection_cursor`) menjadi 3.
  * Percobaan menyuntikkan blok yang melompati urutan (misal: kursor di $H=1$ langsung disuplai $H=3$) wajib ditolak dengan `Err(ProjectionError::NonSequentialBlock { expected: 2, got: 3 })`.
  * Idempotensi: Mengaplikasikan ulang delta $H=2$ ketika kursor sudah di $H=2$ wajib diabaikan secara anggun (*no-op*) atau mengembalikan `Err(ProjectionError::BlockAlreadyProjected)` tanpa memutasi data yang sudah tersimpan.
  * Hash komitmen integritas proyeksi (*projection state digest*) wajib tidak berubah jika delta duplikat disuntikkan.

### [P1] Indeksasi Riwayat Transaksi Berbasis Alamat (Address-Centric Transaction Indexing)

* **Deskripsi:** Modul wajib menyediakan indeks dua arah yang memetakan `AccountId` ke daftar hash transaksi terkait (sebagai pengirim maupun penerima), mendukung pengurutan kronologis terbalik (terbaru lebih dulu) dengan paginasi berbatas (*bounded pagination*).
* **Kriteria Uji:**
  * Eksekusi transaksi transfer dari Alice ke Bob:
    * Alice tercatat memiliki 1 transaksi keluar (*outbound*).
    * Bob tercatat memiliki 1 transaksi masuk (*inbound*).
  * Kueri riwayat dengan filter `limit` dan `offset`:
    * Paginasi wajib bekerja akurat: `limit = 10` mengembalikan tepat maksimal 10 entri.
    * Parameter `limit = 0` atau `limit > MAX_PAGE_LIMIT` (misal $> 100$) wajib dinormalisasi atau ditolak dengan `Err(ProjectionError::InvalidPaginationLimit)`.
  * Kueri riwayat untuk alamat yang tidak pernah bertransaksi wajib mengembalikan daftar kosong `Ok(vec![])` tanpa alokasi memori berlebih.

### [P2] Konsistensi Snapshot Model Baca & Deteksi Lag (Read Model Snapshot Consistency & Lag Tracking)

* **Deskripsi:** Model baca wajib mencerminkan keadaan konsisten pada ketinggian blok tertentu (*Point-in-Time Consistency*). Gateway dapat memantau selisih ketertinggalan proyeksi (*projection lag*) terhadap ketinggian ledger utama.
* **Kriteria Uji:**
  * Jika ledger utama berada di ketinggian $H_{ledger} = 100$ dan proyeksi baru memproses sampai $H_{proj} = 95$, fungsi `projection_lag()` wajib mengembalikan nilai integer `5`.
  * Pembacaan state proyeksi wajib bersifat atomik: pembaca dilarang melihat kondisi setengah terbarui (*partial updates*) saat batch mutasi besar sedang diaplikasikan ke model baca.
  * Kueri pembacaan paralel selama proses konsumsi delta berlangsung tidak boleh mengalami kebuntuan (*deadlock*) atau memicu *race condition*.

### [P3] Batas Retensi & Pembersihan Aman Histori Usang (Bounded Retention & Safe Pruning)

* **Deskripsi:** Untuk mencegah pembengkakan disk (*disk bloat*) dari event log dan receipt transaksi historis, mesin proyeksi mendukung pembersihan (*pruning*) data event di luar jendela retensi (`RETENTION_WINDOW_BLOCKS`) tanpa menghapus atau merusak data saldo aktif akun.
* **Kriteria Uji:**
  * Konfigurasikan jendela retensi $W = 1.000$ blok.
  * Simulasikan rantai hingga blok $H = 1.500$.
  * Jalankan fungsi pembersihan `prune_history(current_height = 1.500)`:
    * Seluruh entri event dan histori transaksi sebelum blok $H = 500$ ($1.500 - 1.000$) terhapus dari tabel indeks histori.
    * Seluruh saldo akun aktif pada $H = 1.500$ wajib tetap utuh dan dapat dikueri secara presisi.
  * Upaya memicu pembersihan dengan parameter ketinggian yang melampaui kursor proyeksi aktif wajib ditolak.

### [P4] Pemulihan Kerusakan & Rekonstruksi Dingin Deterministik (Crash Recovery & Cold Rebuild Determinism)

* **Deskripsi:** Jika database proyeksi dihapus atau mengalami *abrupt shutdown*, modul wajib mampu membangun kembali (*rebuild*) seluruh indeks proyeksi dari nol (*cold rebuild*) dengan membaca ulang rantai blok/delta dari `aurion-ledger`.
* **Kriteria Uji:**
  * Bangun state proyeksi $A$ dengan mengonsumsi 100 blok transaksi secara streaming. Ambil digest komitmen akhir `digest_a = projection_a.digest()`.
  * Simulasikan *cold rebuild* pada instance baru $B$ dengan memutar ulang (*replay*) 100 blok yang sama dari awal secara batch.
  * Verifikasi determinisme mutlak:
    $$
    \text{digest\_a} == \text{digest\_b}
    $$
  * Seluruh indeks saldo, riwayat transaksi per alamat, dan kursor pada instance $B$ wajib identik byte-per-byte dengan instance $A$.

### [P5] Metrik Kinerja & Rasio Kuota Nir-Pecahan (Zero-Float Projection Metrics & BPS Ratio Tracking)

* **Deskripsi:** Seluruh metrik operasional modul—termasuk rasio efisiensi cache (*Cache Hit Ratio BPS*), rata-rata durasi konsumsi per blok (mikrodirektori/milidetik), dan volume memori indeks—wajib dihitung murni menggunakan integer `u64`.
* **Kriteria Uji:**
  * Perhitungan rasio *cache hit*:
    $$
    \text{Cache Hit BPS} = \frac{\text{hits} \times 10.000}{\text{hits} + \text{misses}}
    $$
    dihitung murni secara integer; jika total kueri nol, kembalikan 0 tanpa *division by zero panic*.
  * Akumulasi durasi waktu pemrosesan delta dicatat dalam satuan integer `u64` mikrodetik ($\mu s$) atau milidetik ($ms$).
  * Operasi perhitungan ukuran histori dan pemotongan batas retensi dilindungi oleh `checked_sub` dan `checked_add` untuk mencegah *arithmetic underflow/overflow*.
  * Audit statis: Tidak ada satupun token `f32` atau `f64` pada seluruh berkas sumber di dalam `crates/aurion-projection/src/`.

---

## 4. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian modul ini dikonsolidasikan sebagai suite integrasi internal modul `aurion-projection`:

* **Target Modul:** `crates/aurion-projection/`
* **Lokasi Suite Uji:** `crates/aurion-projection/tests/projection_invariants.rs`
* **Dependensi Pengujian (Dev-Dependencies):**
  * `aurion-core`: Tipe `AccountId`, `Transaction`, `BlockHeader`.
  * `aurion-execution`: Struktur `WriteSet` dan `StateDeltaEntry` untuk pasokan delta.
  * `aurion-ledger`: Penyedia data blok dan tanda terima transaksi.
  * `aurion-criptografi`: Pembangkitan pasangan kunci Ed25519 untuk verifikasi alamat.

---

## 5. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-projection` dinyatakan selesai apabila:

1. Berkas spesifikasi tersimpan di `docs/task-register/TASK-aurion-projection.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji P0-P5 diimplementasikan di `crates/aurion-projection/tests/projection_invariants.rs` dan seluruhnya lulus:
```bash
cargo test -p aurion-projection --test projection_invariants
```

3. Seluruh unit test inline di dalam crate tetap lulus:
```bash
cargo test -p aurion-projection
```

4. Lolos audit kepatuhan statis tanpa error dan warning:
```bash
cargo clippy -p aurion-projection --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check
```

5. Tidak ada penggunaan `unsafe`, tipe `f32`/`f64`, atau pemanggilan `.unwrap()` di jalur produksi `crates/aurion-projection/src/`.
6. Ringkasan temuan dan konfirmasi status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.

---

## 6. RINGKASAN HASIL VERIFIKASI

Verifikasi dijalankan pada workspace dengan perintah di bawah ini, seluruhnya keluar dengan kode 0.

| Perintah | Hasil |
| --- | --- |
| `cargo test -p aurion-projection --test projection_invariants` | 8 passed; 0 failed |
| `cargo test -p aurion-projection` (unit test inline) | 65 passed; 0 failed |
| `cargo clippy -p aurion-projection --all-targets --all-features -- -D warnings` | 0 error; 0 warning |
| `python3 tools/aurion_guard.py check` | Seluruh modul patuh |

### 6.1 Pemetaan Uji Integrasi P0-P5

| Invarian | Testcase |
| --- | --- |
| P0 | `test_p0_ingestion_idempotency` |
| P1 | `test_p1_address_indexing` |
| P2 | `test_p2_snapshot_consistency_and_lag` |
| P3 | `test_p3_pruning_and_retention` |
| P4 | `test_p4_cold_rebuild_is_deterministic_and_crash_recoverable`, `test_p4_cold_rebuild_detects_unavailable_blocks`, `test_p4_cold_rebuild_rejects_tampered_state_root` |
| P5 | `test_p5_zero_float_metrics` |

### 6.2 Catatan Implementasi P4

Implementasi awal P4 hanya consists dari *stub*: `apply_block_to_snapshot` tidak
melakukan apa pun dan selalu mengembalikan `Ok(())`, `get_state_at` mengembalikan
`State::new()` yang kosong, dan `verify_determinism` hanya membandingkan tinggi serta
`state_root`. Tidak satu pun dari ketiga fungsi tersebut memiliki pengujian.

Implementasi sekarang diganti dengan rekonstruksi dingin sungguhan:

1. `cold_rebuild_with_genesis` memutar ulang seluruh transaksi blok pada rentang
   `[from_height, to_height)` ke `aurion_core::State`, lalu memverifikasi bahwa
   `state_root` hasil rekonstruksi identik dengan yang dikomit pada header blok.
   Ketidakcocokan menghasilkan `ProjectionError::StateRootMismatch`.
2. Alokasi genesis diserahkan secara eksplisit melalui parameter `genesis`, karena
   pendanaan akun pada blok genesis tidak dapat diturunkan dari pemutaran ulang
   transaksi (genesis tidak memuat transaksi). `cold_rebuild` tetap disediakan
   sebagai pintasan yang memakai alokasi kosong.
3. Biaya transaksi dikreditkan ke `block.header.proposer`, sama persis dengan
   `Block::execute` di `aurion-core`. Pemutaran ulang yang memakai tujuan biaya
   berbeda akan menghasilkan `state_root` yang menyimpang.
4. `verify_determinism` kini membandingkan peta saldo secara menyeluruh, bukan hanya
   dua skalar, melalui accessor `ReadSnapshot::balances()`.
5. `verify_determinism` tidak memerlukan akses ledger sehingga menjadi *associated
   function* (`ProjectionRecovery::verify_determinism`), sesuai lint
   `clippy::unused_self`.

Testcase P4 membangun ledger `redb` sungguhan berisi 50 blok transfer bertanda tangan,
lalu membandingkan hasil dua *cold rebuild* yang independen terhadap `state_root`
rantai asal beserta saldo kedua pihak.

## 7. Fakta kanonikal model baca saldo (sinkron 2026-09-30)

- Model baca memakai **`Quanta = u128`** untuk seluruh saldo:
  `snapshot.rs`/`ReadSnapshot` memegang `HashMap<AccountId, Quanta>` dan
  `address_index.rs` mengindeks transfer per akun tanpa menurunkan presisi.
- Akses saldo lewat `ReadSnapshot::balances()` (peta menyeluruh, dipakai
  `verify_determinism` P4).
- Purify alokasi genesis dieksplisitkan sebagai parameter `cold_rebuild_with_genesis`
  karena blok genesis tidak memuat transaksi (lih. §6.2).
- Seluruh metrik (cache hit BPS, durasi, lag) tetap `u64`; tidak ada `f32`/`f64`
  pada `crates/aurion-projection/src/`.
