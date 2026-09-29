# TASK: task-aurion-ledger
> **Target Modul:** `aurion-ledger` (`crates/aurion-ledger`)
> **Kategori:** Modul Persistensi & Penyimpanan Keadaan (Persistence Root)
> **Status:** COMPLETED (2026-09-29)
> **Otoritas:** `Agents.md`
> **Backend Database:** `redb` (Pure Rust, ACID, B-Tree)
> **Tingkat Ketergantungan:** 1 (`aurion-core`, `aurion-criptografi`)

---

## 1. Tujuan pengujian

`aurion-ledger` menjadi sumber kebenaran data fisik node. Suite menguji codec dan persistensi melalui API ledger serta backend `redb`; fixture deterministik membuat kegagalan dapat direproduksi.

## 2. Matriks invarian L0–L5 dan hasil

- **L0 — Codec lossless:** account, transaction, block/header round-trip dengan nilai integer batas; encode hasil decode identik. Buffer transaction/block yang terpotong atau memiliki trailing byte ditolak melalui `LedgerError`, tanpa panic. Metadata height dan state root diverifikasi lewat API snapshot. Metadata tidak memiliki codec objek mandiri.
- **L1 — Atomicity/rollback:** tulis beberapa tabel dalam satu transaksi redb, lalu jatuhkan transaksi sebelum commit. Setelah reopen, account, block, dan metadata tetap pada baseline.
- **L2 — Isolasi partisi:** akun, block hash index, block, metadata, dan module KV disimpan dalam tabel redb terpisah. Tes memakai byte key yang sama pada akun, module KV, dan hash index, serta memeriksa lookup/range tiap tabel.
- **L3 — Append-only/gapless:** genesis tinggi 0 dan blok berikutnya H+1 diterima. Tinggi lompat menghasilkan `NonSequentialBlock`, parent hash yang bercabang menghasilkan `PreviousHashMismatch`, dan overwrite menghasilkan `BlockAlreadyExists`. State pemanggil tidak berubah saat commit ditolak. API ledger tidak menyediakan operasi hapus/overwrite blok.
- **L4 — Snapshot CQRS:** `LedgerSnapshot` mempertahankan pembacaan konsisten saat thread lain melakukan commit H+1 beserta mutasi akun; snapshot baru melihat versi terbaru. Write diselesaikan ketika snapshot lama tetap hidup.
- **L5 — Reopen/recovery:** temporary database menyimpan blok 0–10; sesudah instance ditutup dan dibuka kembali, semua blok, latest height, state root terakhir, dan akun terhidrasi sama.

## 3. Lokasi dan dependensi suite

- **Suite:** `crates/aurion-ledger/tests/ledger_invariants.rs`
- **Implementasi:** `crates/aurion-ledger/src/`
- `tempfile` sudah menjadi dev-dependency. `aurion-core`, `aurion-criptografi`, dan `redb` dipakai melalui dependensi crate yang sudah ada; tidak ada dependensi bisnis baru.

## 4. Perubahan runtime yang diperlukan

- Menambahkan `MODULE_KV_TABLE` sebagai partisi tabel key/value untuk data modul.
- Memvalidasi tinggi blok dan menolak duplikasi/tinggi tidak berurutan dengan error bertipe.
- Mengeksekusi blok pada salinan `State`; state pemanggil diperbarui hanya setelah transaksi redb berhasil commit.
- Menyediakan `LedgerSnapshot` untuk kueri multi-tabel konsisten dan akses latest state root.

## 5. Definition of Done

Task selesai setelah suite integrasi, unit test inline, Clippy ketat, dan security guard lulus:

```bash
cargo test -p aurion-ledger --test ledger_invariants
cargo test -p aurion-ledger
cargo clippy -p aurion-ledger --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check
```

Semua perintah tersebut lulus pada 2026-09-29. Ringkasan tersimpan di `logs/WORK_LOG.md`.
