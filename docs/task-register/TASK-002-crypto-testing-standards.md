# TASK-002: Standar Uji Kriptografi Aurion (C0–C7 Verification Framework)

* **Modul Terkait:** `crates/aurion-criptografi` (`aurion-criptografi`)
* **Versi Spesifikasi:** 1.0.0
* **Status Siklus Verifikasi:** `DRAFT`
* **Prinsip Utama:** Primitive Benar != Construction Aurion Benar. Larangan mempercayai dependensi eksternal secara buta tanpa pembuktian diferensial independen.

> Catatan penamaan: direktif awal menyebut `crates/crypto` (`aurion-crypto`).
> Nama yang dikunci pada workspace ini adalah `crates/aurion-criptografi`
> (`aurion-criptografi`). Dokumen ini memakai nama terkunci tersebut.

---

## 1. Siklus Hidup Rilis (Verification Gate Pipeline)

Setiap implementasi primitif kriptografi maupun konstruksi tingkat tinggi wajib melalui transisi status berurutan secara linier. Tidak ada komponen yang boleh digunakan oleh modul lain jika belum mencapai status `APPROVED`.

```text
[ DRAFT ]
    │
    ▼
[ IMPLEMENTED ] ───────────► (Lulus C0: Lint, Format, Build Bersih)
    │
    ▼
[ VECTOR-VERIFIED ] ───────► (Lulus C1: Official Vectors byte-per-byte)
    │
    ▼
[ PROPERTY-VERIFIED ] ─────► (Lulus C2 & C3: Invariant & Negative Test)
    │
    ▼
[ DIFFERENTIAL-VERIFIED ] ─► (Lulus C4: Oracle Equivalence vs Reference)
    │
    ▼
[ FUZZ-VERIFIED ] ─────────► (Lulus C5: Crash/Panic/OOM Free via libFuzzer)
    │
    ▼
[ DETERMINISM-VERIFIED ] ──► (Lulus C6: Multi-thread Rayon Equivalence)
    │
    ▼
[ SECURITY-REVIEWED ] ─────► (Lulus C7: Constant-time & Memory Zeroize Audit)
    │
    ▼
[ APPROVED ] ──────────────► Siap masuk jalur produksi / main branch
```

**Aturan transisi status:** transisi ke tahap berikutnya hanya sah jika bukti
eksekusi log pengujian terlampir (perintah yang dijalankan, exit code, dan
ringkasan hasil). Tanpa bukti log, status tetap pada tahap berjalan.

---

## 2. Matriks 8 Tingkat Pengujian (C0 – C7)

| Level | Kategori Pengujian | Sasaran & Cakupan Teknis | Alat / Runner | Status Wajib |
| --- | --- | --- | --- | --- |
| **C0** | **Rust Code Integrity** | Sanitasi dependensi, format kode, zero-warning, proteksi unsafe code. | `cargo fmt`, `clippy -D warnings`, `cargo deny`, `cargo audit` | **Wajib Mutlak** |
| **C1** | **Known Answer Test (KAT)** | Output byte-per-byte vs vektor resmi (Official Test Vectors). | `cargo test --test vectors` | **Wajib Mutlak** |
| **C2** | **Property-Based Testing** | Pelanggaran invariant matematis via ribuan input pseudo-acak. | `proptest`, `quickcheck` | **Wajib Mutlak** |
| **C3** | **Negative & Boundary Test** | Menolak data cacat, kunci korup, payload terpotong, format invalid. | `cargo test --test negative` | **Wajib Mutlak** |
| **C4** | **Differential Testing** | Hasil Aurion vs reference implementation pihak ketiga (*oracle*). | Test suite multi-backend | **Wajib Mutlak** |
| **C5** | **Fuzz Testing** | Input acak ekstrim: cegah panic, infinite loop, OOM. | `cargo-fuzz` (libFuzzer) | **Wajib Mutlak** |
| **C6** | **Determinism & Concurrency** | Rayon 1..N thread menghasilkan status kriptografis identik. | `rayon`, thread-scaling harness | **Wajib Mutlak** |
| **C7** | **Security & Hardware Gate** | Audit constant-time, zeroize RAM, isolasi side-channel. | `dudect`, `subtle`, valgrind | **Wajib Rilis** |

---

## 3. Rincian Eksekusi & Kriteria Kelulusan

### C0 — Rust Integrity

* **Perintah:**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check advisories bans licenses sources
cargo audit
```

* **Kriteria Lulus:** Exit code 0, nol warning, nol dependensi rentan.

### C1 — Known Answer Test (Vektor Resmi)

* **BLAKE3:** Wajib memakai vektor resmi tim BLAKE3 (32-byte hash, keyed hash, derive key).
* **Ed25519:** Wajib memakai vektor RFC 8032 (baseline Ed25519, signature 64-byte, raw public key).
* **Larangan:** Dilarang assertion ambigu seperti `assert!(sig.is_ok())`.
* **Kriteria Lulus:** `assert_eq!(actual_bytes, expected_official_vector_bytes)` mutlak.

### C2 — Invariant Property Testing

* Minimal 10.000 kombinasi state otomatis.
* **Properti BLAKE3:**

```text
StreamingHash(m) == OneShotHash(m) AND H(m) == H(m)
```

* **Properti Ed25519:**

```text
Verify(pk, m, Sign(sk, m)) = true
Untuk semua m' != m: Verify(pk, m', Sign(sk, m)) = false
```

### C3 — Negative Testing (Penolakan Ketat)

* Tanda tangan panjang != 64 bita -> wajib `Err(CryptoError::InvalidSignature)`.
* Kunci publik panjang != 32 bita -> wajib `Err(CryptoError::InvalidPublicKey)`.
* Tanda tangan sah dimodifikasi 1 bit -> wajib ditolak.
* Kunci publik non-kanonikal (titik kurva tidak valid) -> wajib ditolak.

### C4 — Differential Testing (Multi-Implementation Oracle)

Input yang sama dialirkan paralel ke tiga target:

1. Implementasi Produksi Aurion (optimized path via Rayon/SIMD).
2. Reference Implementation resmi BLAKE3 (C reference / simple Rust path).
3. Pustaka independen pihak ketiga yang terisolasi.

* **Kriteria Lulus:** `Output_A == Output_B == Output_C`.

### C5 — Fuzz Testing

* Target pengujian:
  * Parser tanda tangan dan kunci publik.
  * Ingestion streaming BLAKE3 panjang payload 0 hingga 64 MB.
  * Batch verifier input array mismatch (beda jumlah pesan vs signature).
* **Kriteria Lulus:** Fuzzing minimal 1 jam tanpa crash, tanpa `panic!`, tanpa memory leak.

### C6 — Determinism & Concurrency (Rayon Invariance)

* Menguji N = 2048 transaksi acak.
* Batch verifier diuji dengan variasi core:

```text
Verify_single_thread(T) == Verify_rayon(T, threads in {1, 2, 4, 8, 16})
```

* **Kriteria Lulus:** Hasil boolean per transaksi tidak bergeser akibat race condition, urutan chunking Rayon, atau level optimasi compiler.

### C7 — Security & Performance Gate

* **Zeroization:** struct kunci privat menimpa RAM dengan bita `0x00` saat `drop()`.
* **Constant-Time Verification:** uji durasi `dudect`, nihil timing leakage di jalur kritis.
* **Catatan Kritis:** throughput tinggi (misal 100k verifikasi/detik) **bukan** bukti aman. Performa hanya diukur setelah C1–C6 lulus 100%.

---

## 4. Pemisahan Pengujian: Primitive vs Aurion Construction

Dilarang menyamakan pengujian algoritma mentah dengan konstruksi Aurion:

```text
crates/aurion-criptografi/
├── tests/
│   ├── primitives/                # Uji Algoritma Mentah (Isolated)
│   │   ├── blake3_official_kat.rs # C1 RFC/Official vectors
│   │   ├── ed25519_rfc8032.rs     # C1 RFC 8032 vectors
│   │   └── properties.rs          # C2 Mathematical invariants
│   │
│   └── constructions/             # Uji Mekanisme Internal Aurion
│       ├── canonical_encoding.rs  # Determinisme bita sebelum hashing
│       ├── domain_separation.rs   # Anti-collision cross-protocol hash
│       ├── batch_verifier.rs      # Isolasi racun (poisoned batch fallback)
│       ├── memory_zeroize.rs      # Valgrind memory inspection
│       └── thread_determinism.rs  # Rayon scaling invariability
```

---

## 5. Definition of Done (DoD) untuk TASK-002

1. Folder struktur pengujian `tests/primitives/` dan `tests/constructions/` terpasang.
2. Vektor uji RFC 8032 dan BLAKE3 disematkan sebagai fixture biner/JSON mentah di repository.
3. Seluruh suite `cargo test -p aurion-criptografi` lulus level C0 hingga C3.
4. CI pipeline mengeksekusi C0 hingga C6 pada setiap pull request ke modul inti.


