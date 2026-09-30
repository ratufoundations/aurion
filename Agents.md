# AURION PROTOCOL — AGENTS RULEBOOK & ARCHITECTURE AUTHORITY
> **Status:** Otoritatif & Mengikat (Active)  
> **Revisi Terakhir:** 2026-09-30  
> **Lingkup:** Seluruh AI Agent, Kontributor, dan Kode di Workspace `aurion/`

---

## 1. HUKUM KEAMANAN NON-NEGOSIASI (HARD INVARIANTS)

Setiap kode yang dihasilkan agen WAJIB mematuhi empat aturan mutlak berikut tanpa pengecualian:

1. **LARANGAN MUTLAK KODE UNSAFE (`forbid(unsafe_code)`):**
   * Tidak boleh ada blok `unsafe { ... }` di seluruh crate maupun aplikasi.
   * Setiap berkas `lib.rs` dan `main.rs` wajib mendeklarasikan `#![forbid(unsafe_code)]` di baris pertama.

2. **LARANGAN MUTLAK BILANGAN PECAHAN (`zero-float`):**
   * Tipe `f32` dan `f64` dilarang keras di seluruh workspace. Penegakan dilakukan oleh lint Clippy workspace di `Cargo.toml`, daftar `disallowed-types` di `clippy.toml`, dan pemindai `tools/aurion_guard.py`; `deny.toml` mengatur supply chain, bukan tipe numerik.
   * Seluruh kalkulasi finansial, rasio, kuorum, atau persentase wajib menggunakan fixed-point integer (`u128` / satuan Quanta). 
   * 1 AUR = 10^10 Quanta (10.000.000.000). Operasi pembagian wajib mempertimbangkan integer truncation dan sisa bagi (`%`).

3. **LARANGAN MEMBUAT RODA SENDIRI (STANDAR PUSTAKA MATANG):**
   * Dilarang mengimplementasikan ulang fungsi dasar (kriptografi, network framing, CLI parser, UI table, serialization).
   * Wajib memanfaatkan crate industri yang teruji, matang, dan lolos audit (misal: `zenoh`, `redb`, `clap`, `comfy-table`, `blake3`, `ed25519-dalek`, `tokio`, `tracing`, `toml`, `serde`).

4. **ANTI-PANIC & TYPE SAFETY:**
   * Dilarang menggunakan `.unwrap()` di jalur eksekusi produksi/kritis konsensus.
   * Gunakan penanganan galat terstruktur bertipe kuat (`thiserror` untuk crate internal, `anyhow`/`Box<dyn Error>` hanya di tingkat aplikasi `main.rs`).

---

## 2. PEMBAGIAN WILAYAH WORKSPACE (SEPARATION OF CONCERNS)

Workspace Aurion dibagi secara kaku ke dalam tiga lapisan:


```

aurion/
├── config/              [AUTHORITY] Konfigurasi terpusat (aurion-config) & Logger init
├── crates/ (13 crates)  [PARTS]     Pustaka murni: Konsensus, Ledger, Core, Guard, dll.
└── apps/                [MACHINES]  Biner eksekusi: Node, Explorer, CLI.

```

### A. Lapisan Konfigurasi (`config/`)
* Bertindak sebagai otoritas tunggal nilai baku, schema TOML, dan inisialisasi logging global.
* Menampung `node.dev.toml` dan `node.prod.toml`.
* Semua aplikasi membaca konfigurasi melalui crate `aurion-config`.

### B. Lapisan Pustaka Murni (`crates/*`)
* **DILARANG** membaca berkas file system (`.toml`, `.json`) secara mandiri.
* **DILARANG** menginisialisasi logger (`init()`).
* **HANYA** menerima parameter bertipe data murni Rust dari pemanggil (misal: `&Path`, `u64`, `Keypair`).
* **HANYA** memanggil instrumen makro log `tracing::{trace, debug, info, warn, error}` (Zero-Overhead Producer).

### C. Lapisan Biner Aplikasi (`apps/*`)
* Merakit pustaka-pustaka dari `crates/*` menjadi simpul atau alat fungsional.
* Menjadi satu-satunya pihak yang memuat `aurion-config` dan mengaktifkan logger runtime di awal `main()`.

---

## 3. MODE OPERASIONAL & ATURAN LOGGING

Sistem Aurion hanya mengenal 2 mode operasional:

| Parameter | Mode Developer (`developer`) | Mode Production (`production`) |
|---|---|---|
| **Stdout / Terminal** | Verbose (Warna ANSI, Line number, Target) | **Hening (Silent)**. Tidak ada polusi I/O. |
| **Output File** | `logs/{app}-dev.log` (Level DEBUG) | `logs/{app}-prod.log` (Level ERROR saja) |
| **Tujuan** | Debugging alur logika & state | Throughput konsensus & efisiensi CPU maksimal |

Agen dilarang menggunakan `println!` atau `eprintln!` untuk pencatatan state internal mesin di dalam `crates/`. Gunakan `tracing`.

---

## 4. PROTOKOL KERJA AGEN (STEP-BY-STEP WORKFLOW)

Ketika menerima instruksi teknis dari pengguna, agen wajib bekerja dengan metodologi berikut:

1. **Prinsip Minimalis & Fondasi Terlebih Dahulu:**
   * Jangan melompat ke fitur masa depan sebelum fondasi inti tuntas dan teruji.
   * Jaga codebase tetap seringkas mungkin (menghindari abstraksi kosong atau over-engineering).

2. **Isolasi Tugas (Pemisahan Instruksi):**
   * Jangan mencampur aduk pekerjaan fondasi keamanan/lints dengan implementasi UI/CLI.
   * Kerjakan satu fokus modul per instruksi agar mudah diverifikasi dan dilacak riwayat perubahannya.

3. **Verifikasi Kepatuhan Sebelum Menyatakan Selesai:**
   Setiap perubahan kode harus diverifikasi melalui rangkaian perintah berikut:
   ```bash
   # 1. Pastikan kompilasi sukses
   cargo check --workspace

   # 2. Pastikan patuh aturan unsafe, zero-float, dan lints ketat
   cargo clippy --workspace --all-targets --all-features -- -D warnings

   # 3. Jalankan unit test internal
   cargo test -p <crate_yang_diubah>

```

4. **Pemeriksaan Guard Wajib:**
   * Setiap kali `tools/aurion_guard.py` dibuat, diubah, atau diperbaiki, jalankan `python3 tools/aurion_guard.py check` dari root repositori.
   * Jika pemindaian gagal atau melaporkan pelanggaran, telusuri dan perbaiki penyebabnya, lalu jalankan ulang pemeriksaan. Ulangi sampai skrip keluar dengan status sukses dan tidak melaporkan pelanggaran.
   * Jangan menyatakan pekerjaan pada `tools/aurion_guard.py` selesai sebelum pemeriksaan tersebut berhasil.

5. **Catatan Kerja & Kontinuitas Agen:**
   * Sebelum mulai bekerja, setiap agen wajib membaca `logs/WORK_LOG.md` untuk memahami pekerjaan terbaru, keputusan yang sudah dibuat, hasil verifikasi, dan tindak lanjut yang masih terbuka.
   * Setelah menyelesaikan pekerjaan, setiap agen wajib menambahkan entri terbaru di bagian atas `logs/WORK_LOG.md` yang mencatat tanggal, ringkasan pekerjaan, berkas yang diubah, pemeriksaan yang dijalankan beserta hasilnya, dan tindak lanjut yang masih diperlukan. Jangan menghapus riwayat entri sebelumnya.
   * Jangan mencatat rahasia, token, atau kredensial di log.

---

## 5. INVARIAN PROTOKOL & TATA KELOLA KONSENSUS

* **Pasokan Moneter Genesis:** 66.000.000 AUR (= 660.000.000.000.000.000 Quanta, `TREASURY_GENESIS_QUANTA`) terkunci di Treasury pada Blok 0 (Genesis).
* **Dewan Guard:** 5 anggota dewan kuorum, razia darurat wajib aklamasi 5/5, pemilihan rotasi epoch berbasis skor kontribusi dan stake.
* **Penerimaan Validator Baru:** Wajib masa probation 1 minggu (7 hari heartbeat liveness $\ge 99\%$) + surat dukungan kriptografis dari minimal 3 validator aktif.
* **Konsensus BFT:** Ambang batas finalisasi blok adalah kuorum super-mayoritas $2f + 1$.
* **Penyimpanan:** Format serialisasi biner kanonikal deterministik berbasis B-Tree ACID (`redb`).
* **Arah modul komposabel:** `docs/RFC-001.md` adalah acuan desain yang diusulkan untuk isolasi capability-keeper, cache transaksi atomik, dan envelope transaksi umum. Status RFC dan fase implementasi di dalam dokumen tetap berlaku; jangan menganggap API atau fase yang belum diimplementasikan sebagai perilaku runtime yang sudah tersedia.
