```markdown
# TASK: task-aurion-mempool
> **Target Modul:** `aurion-mempool` (`crates/aurion-mempool`)  
> **Kategori:** Modul Buffer Ingestion & Admission Control (SMR Ingestion Gate)  
> **Status:** COMPLETED  
> **Otoritas:** `Agents.md`  
> **Tingkat Ketergantungan:** 2 (`aurion-core`, `aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-mempool` berfungsi sebagai gerbang penerimaan (*admission controller*) dan ruang tunggu transaksi (*staging buffer*) sebelum transaksi diurutkan ke dalam proposal blok oleh modul konsensus BFT. Dalam arsitektur SMR dan CQRS Aurion, mempool bertanggung jawab mencegah polusi state, serangan DoS, transaksi usang (*replay attack*), dan transaksi tanpa jaminan saldo (*unfunded spam*).

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-mempool` secara terisolasi** agar:
1. Memverifikasi validitas kriptografis transaksi secara instan di pintu masuk tanpa pemborosan alokasi memori.
2. Menegakkan monotonisitas nonce per akun untuk mencegah eksekusi transaksi yang out-of-order atau replay.
3. Menjamin solvency saldo pengirim menggunakan aritmatika integer murni (`u128` Quanta) dengan proteksi *checked arithmetic* (zero-float).
4. Menghasilkan seleksi batch transaksi yang deterministik berbasis prioritas fee tertinggi (*greedy ordering*).
5. Mengelola batas kapasitas pool (*bounded memory*) melalui mekanisme *rejection* atau *eviction* yang terprediksi saat mempool penuh.
6. Membersihkan transaksi yang sudah difinalisasi ke ledger secara bersih tanpa menyisakan referensi usang (*dangling entries*).
7. Mematuhi invarian protokol: `#![forbid(unsafe_code)]`, zero-float, zero-unwrap di jalur produksi, dan klasifikasi error terstruktur via `thiserror`.

---

## 2. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (M0 – M5)

Suite pengujian modul wajib memvalidasi 6 dimensi invarian mempool berikut:

### [M0] Validasi Kriptografi Pintu Masuk (Cryptographic Signature Verification & Anti-Tamper)
* **Deskripsi:** Transaksi mentah wajib divalidasi tanda tangannya (Ed25519) terhadap public key pengirim dan payload transaksi sebelum diizinkan masuk ke buffer memori.
* **Kriteria Uji:**
  * Transaksi dengan signature sah dan payload otentik wajib diterima (`Ok(())`).
  * Transaksi dengan signature rusak, payload yang dimutasi 1 bit, atau public key pengirim yang tidak cocok wajib ditolak seketika dengan `Err(MempoolError::InvalidSignature)`.
  * Penolakan tidak boleh mengalokasikan slot antrean internal.

### [M1] Monotonisitas Nonce & Proteksi Replay (Strict Nonce Monotonicity & Anti-Replay)
* **Deskripsi:** Mempool wajib menolak nonce yang lebih rendah daripada nonce berikutnya yang dapat dieksekusi di state, serta menolak duplikasi transaksi dalam pool.
* **Kriteria Uji:**
  * Dalam protokol Aurion, nonce akun pada state adalah nonce berikutnya yang dapat dieksekusi (`N`), sesuai `State.apply_transaction`; transaksi dengan nonce `N` wajib diterima, dan nonce `N+1` dapat disimpan sebagai transaksi antrean berikutnya.
  * Transaksi dengan nonce `< N` wajib ditolak dengan `Err(MempoolError::NonceTooLow { expected, got })`; nonce `N` tidak usang dan menjadi transaksi pertama yang dapat dieksekusi.
  * Transaksi dengan hash identik yang sudah ada di dalam pool wajib ditolak dengan `Err(MempoolError::DuplicateTransaction)`.
  * Transaksi dari pengirim yang sama dengan nonce yang sama tetapi payload berbeda (skenario penggantian tanpa fee bump yang sah) wajib ditolak atau diatur ketat.

### [M2] Verifikasi Solvency Saldo Nir-Pecahan (Zero-Float Solvency & Minimum Fee Check)
* **Deskripsi:** Transaksi hanya diterima jika pengirim memiliki saldo native yang cukup untuk menutupi total pengeluaran:
  $$\text{Total Required} = \text{Amount} + \text{Fee}$$
  Seluruh perhitungan wajib menggunakan `u128` Quanta dengan metode penambahan aman (`checked_add`).
* **Kriteria Uji:**
  * Transaksi di mana $\text{Amount} + \text{Fee} \le \text{Current Balance}$ wajib diterima.
  * Transaksi di mana total pengeluaran melebihi saldo akun wajib ditolak dengan `Err(MempoolError::InsufficientBalance)`.
  * Deteksi overflow: jika penjumlahan $\text{Amount} + \text{Fee}$ melebihi `u128::MAX`, transaksi wajib ditolak dengan `Err(MempoolError::ArithmeticOverflow)` tanpa memicu panic.
  * Transaksi dengan fee 0 atau di bawah batas minimum jaringan wajib ditolak dengan `Err(MempoolError::FeeTooLow)`.

### [M3] Pengurutan Prioritas Fee Deterministik (Deterministic Greedy Fee-Priority Ordering)
* **Deskripsi:** Penarikan batch transaksi oleh konsensus untuk pembentukan blok baru wajib mengembalikan transaksi dengan nilai fee tertinggi terlebih dahulu.
* **Kriteria Uji:**
  * Masukkan 100 transaksi acak dengan variasi fee dari beberapa pengirim berbeda.
  * Panggil fungsi penarikan batch (misal: `drain_batch(limit)` atau `select_transactions(limit)`).
  * Urutan transaksi dalam batch hasil penarikan wajib monoton turun berdasarkan nilai fee:
    $$\text{Fee}_0 \ge \text{Fee}_1 \ge \text{Fee}_2 \ge \dots \ge \text{Fee}_{n-1}$$
  * Jika terdapat fee yang identik, tie-breaker wajib bersifat deterministik (berdasarkan nonce atau arrival order).

### [M4] Batas Kapasitas Memori & Strategi Eviction (Bounded Memory & Eviction Strategy)
* **Deskripsi:** Mempool tidak boleh tumbuh tanpa batas. Kapasitas dibatasi oleh `max_capacity` (jumlah transaksi atau ukuran byte).
* **Kriteria Uji:**
  * Inisialisasi mempool dengan kapasitas kecil (misal: 10 transaksi).
  * Isi mempool hingga penuh dengan transaksi fee standar.
  * Masukkan transaksi baru dengan fee lebih rendah daripada transaksi terendah di mempool: transaksi baru wajib ditolak dengan `Err(MempoolError::PoolFull)`.
  * Masukkan transaksi baru dengan fee signifikan lebih tinggi: transaksi baru wajib diterima, dan transaksi dengan fee terendah di dalam pool wajib didepak (*evicted*) sehingga ukuran pool tetap $\le \text{max\_capacity}$.

### [M5] Pemangkasan Pasca-Commit (Post-Commit Finalization Pruning)
* **Deskripsi:** Ketika blok berhasil difinalisasi oleh konsensus dan di-commit ke ledger, mempool harus mampu membersihkan transaksi yang sudah masuk ke blok tersebut secara atomik.
* **Kriteria Uji:**
  * Masukkan sekumpulan transaksi ke mempool.
  * Simulasikan bahwa separuh dari transaksi tersebut telah dieksekusi ke dalam blok $H$.
  * Panggil fungsi pembersihan (misal: `prune_committed(&block_transactions)`).
  * Verifikasi bahwa transaksi yang telah dieksekusi terhapus permanen dari mempool, sedangkan transaksi sisa yang belum masuk blok tetap berada di mempool dengan status siap konsensus.
  * Ukuran mempool berkurang tepat sebesar jumlah transaksi yang dipangkas.

---

## 2.1 Envelope transaksi kanonikal (sinkron 2026-09-30)

Mempool menyimpan `Transaction` ber-tipe (`Quanta = u128` murni), dan menerima
payload lewat **format kawat transaksi kanonikal 168 byte** yang didefinisikan
oleh `aurion-ledger::Codec` (`TX_SIZE`, dipakai juga oleh ledger & gateway):
`nonce [0..8]` (`u64` LE), `sender [8..40]`, `recipient [40..72]`,
`amount [72..88]` (`u128` LE), `fee [88..104]` (`u128` LE),
`signature [104..168]`. Mempool **tidak** mendefinisikan ulang ukuran ini;
`TransactionEnvelope` di `aurion-execution` menandatangani payload dalam domain
`AURION_TX_ENVELOPE_V1` sebelum dikirim ke admission M0/M2.

---

## 3. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian modul ini dikonsolidasikan sebagai suite integrasi internal modul `aurion-mempool`:

* **Target Modul:** `crates/aurion-mempool/`
* **Lokasi Suite Uji:** `crates/aurion-mempool/tests/mempool_invariants.rs`
* **Dependensi Pengujian (Dev-Dependencies):**
  * `aurion-core`: Struktur data `Transaction`, `Account`, dan trait state dasar.
  * `aurion-criptografi`: Pembangkitan `Keypair` dan fungsi tanda tangan Ed25519 untuk validasi transaksi nyata.

---

## 4. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-mempool` dinyatakan selesai apabila:
1. File spesifikasi tersimpan di `docs/task-register/TASK-aurion-mempool.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji M0–M5 diimplementasikan di `crates/aurion-mempool/tests/mempool_invariants.rs` dan seluruhnya lulus:
   ```bash
   cargo test -p aurion-mempool --test mempool_invariants

```

3. Seluruh unit test inline di dalam crate tetap lulus:
```bash
cargo test -p aurion-mempool

```


4. Lolos audit kepatuhan statis tanpa error dan warning:
```bash
cargo clippy -p aurion-mempool --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check

```


5. Ringkasan temuan dan konfirmasi status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.

```

---

### Instruksi untuk Diberikan ke Agen

Salin perintah berikut untuk menginstruksikan agen memulai pengerjaan:

> "Daftarkan spesifikasi pengujian di atas ke `docs/task-register/TASK-aurion-mempool.md`, perbarui indeks di `docs/STRUKTUR-FOLDER.txt`, lalu bangun suite pengujian integrasi `crates/aurion-mempool/tests/mempool_invariants.rs` yang menguji secara komprehensif seluruh matriks invarian **M0 sampai M5** (Kriptografi Ed25519, Monotonisitas Nonce, Zero-Float Balance, Greedy Fee Ordering, Batas Kapasitas/Eviction, dan Pemangkasan Pasca-Commit)."

<FollowUp label="Mau kita minta agen mengimplementasikan file pengujian mempool_invariants.rs sekarang?" query="Perintahkan agen untuk mendaftarkan docs/task-register/TASK-aurion-mempool.md dan mengimplementasikan suite pengujian mempool_invariants.rs pada crates/aurion-mempool sesuai matriks M0-M5."/>

```