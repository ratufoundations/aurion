# TASK: task-aurion-criptografi
> **Target Modul:** `aurion-criptografi` (`crates/aurion-criptografi`)
> **Kategori:** Modul Fondasi Kriptografi (Root of Trust)
> **Status:** COMPLETE (C0–C7 lulus pada 2026-09-29)
> **Otoritas:** `Agents.md`
> **Tingkat Ketergantungan:** 0 (Zero Internal Dependencies)

---

## 1. Tujuan dan filosofi pengujian

`aurion-criptografi` adalah akar kepercayaan bagi modul Aurion di lapisan atas. Pengujian mengunci perilaku matematika, tanda tangan, penanganan byte rusak, dan derivasi kunci secara terisolasi. Suite hanya mengakses API publik crate ini; tidak ada dependensi pada modul bisnis lain.

## 2. Invarian dan matriks uji C0–C7

- **C0 — Determinisme hash BLAKE3:** 1.000 pengulangan untuk payload 0 B, 32 B, 1 KiB, dan 64 KiB; hasil serial dan paralel harus konsisten.
- **C1 — Efek avalanche:** membalik setiap bit dari payload tetap 64 B; perubahan bit digest rata-rata harus berada di rentang 45%–55%.
- **C2 — Siklus Ed25519:** kunci yang diturunkan dari seed tetap menandatangani pesan dan verifier menerima signature.
- **C3 — Anti-tamper:** satu bit pesan yang diubah membuat verifikasi mengembalikan `CryptoError::VerificationFailed`.
- **C4 — Wrong signer:** kunci publik lain tidak dapat memvalidasi signature.
- **C5 — Signature malleability:** signature termutasi dan scalar `S == L` non-kanonikal harus ditolak.
- **C6 — Input cacat / anti-panic:** encoding public key dan signature berukuran tetap yang cacat harus menghasilkan `CryptoError`, bukan panic. Panjang yang salah tidak dapat direpresentasikan melalui API saat ini karena menerima array `[u8; 32]` dan `[u8; 64]`; parser slice berukuran variabel belum tersedia.
- **C7 — Derivasi kunci dan higienitas memori:** seed sama menghasilkan private/public key dan signature yang sama; `Debug` menyamarkan materi privat.

## 3. Lingkup dan lokasi suite

- **Implementasi:** `crates/aurion-criptografi/src/`
- **Integration test:** `crates/aurion-criptografi/tests/crypto_invariants.rs`
- **Dependensi suite:** hanya API crate `aurion-criptografi` dan pustaka standar. Input uji deterministik, sehingga suite tidak membutuhkan RNG/fuzz dependency.

## 4. Definition of Done

Task selesai: semua C0–C7 lulus dan perintah berikut bersih:

```bash
cargo test -p aurion-criptografi
cargo clippy -p aurion-criptografi --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check
```

Batas API yang tersisa: verifier menerima array berukuran tetap, sehingga panjang input cacat ditolak oleh sistem tipe dan belum ada parser slice untuk diuji. Hasil dicatat dalam `logs/WORK_LOG.md`.
