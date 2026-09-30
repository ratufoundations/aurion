```markdown
# TASK: task-aurion-core
> **Target Modul:** `aurion-core` (`crates/aurion-core`)  
> **Kategori:** Fungsi Transisi Keadaan Murni & Tipe Data Inti (State Machine Engine)  
> **Status:** COMPLETED — ST0–ST5 fee bertanda tangan dan timestamp monotonik terverifikasi penuh  
> **Otoritas:** `Agents.md`  
> **Tingkat Ketergantungan:** 1 (`aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-core` adalah mesin transisi keadaan matematis murni (*Pure State Transition Function*) dari Protokol Aurion:
$$S_{n+1} = \text{Apply}(S_n, \text{Block})$$

Modul ini mendefinisikan tipe data kanonikal (`Block`, `BlockHeader`, `Transaction`, `AccountState`, `StateRoot`) dan mengimplementasikan aturan mutasi deterministik. Di sinilah keputusan konsensus diwujudkan menjadi perubahan saldo, kenaikan nonce, dan komitmen kriptografis *state root*. `aurion-core` dilarang menyentuh I/O jaringan atau disk secara langsung; semua operasi berlangsung murni pada struktur data memori.

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-core` secara terisolasi** agar:
1. Memastikan fungsi transisi keadaan menghasilkan komitmen *State Root* BLAKE3 yang 100% deterministik dan dapat direplikasi (*replay-ready*).
2. Menjamin eksekusi transaksi dalam blok bersifat atomik—blok yang cacat tidak boleh meninggalkan mutasi parsial pada state.
3. Menegakkan invarian konservasi saldo (*solvency*): total koin terjaga, mutasi saldo anti-underflow/overflow, dan menggunakan integer murni `u128` Quanta.
4. Memastikan kenaikan nonce akun tepat monotonik ($N \to N+1$) per transaksi yang dieksekusi.
5. Memverifikasi integritas struktural blok: kelanjutan tinggi blok ($H+1$), tautan *parent hash*, dan konsistensi pohon transaksi/state.
6. Mematuhi invarian protokol: `#![forbid(unsafe_code)]`, zero-float, zero-unwrap di jalur produksi, serta penanganan error menggunakan tipe kuat via `thiserror`.

---

## 2. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (ST0 – ST5)

Suite pengujian modul wajib memvalidasi 6 dimensi invarian transisi keadaan berikut:

### [ST0] Determinisme Fungsi Transisi Keadaan & State Root (State Root Determinism & Replay Invariance)
* **Deskripsi:** Menjalankan transisi keadaan dari $S_0$ menggunakan urutan blok yang sama $[B_1, B_2, \dots, B_k]$ wajib menghasilkan hash `StateRoot` BLAKE3 yang sama persis di setiap simpul dan di setiap proses replay independen.
* **Kriteria Uji:**
  * Inisialisasi dua instance state memori terpisah ($S_A$ dan $S_B$) dengan state awal identik.
  * Terapkan 50 transaksi acak valid yang dikemas dalam beberapa blok ke $S_A$ dan $S_B$.
  * State Root akhir $S_A$ dan $S_B$ wajib identik byte-per-byte (`assert_eq!(root_a, root_b)`).
  * Replay: Simulasikan pemutaran ulang blok dari nol; state root hasil rekalkulasi wajib cocok 100% dengan state root pada header blok.

### [ST1] Atomisitas Eksekusi Blok (Block Execution Atomicity & Rollback Safety)
* **Deskripsi:** Sebuah blok dieksekusi dengan prinsip *all-or-nothing*. Jika salah satu transaksi di dalam blok tidak valid (misal: saldo tidak cukup atau signature korup), seluruh mutasi blok tersebut wajib dibatalkan tanpa mencemari state aktif.
* **Kriteria Uji:**
  * Siapkan blok berisi 5 transaksi: Transaksi #1 sampai #4 valid, Transaksi #5 sengaja dibuat tidak valid (misal: penarikan dana melebihi saldo akun).
  * Jalankan eksekusi blok. Eksekusi wajib mengembalikan `Err(StateTransitionError::...)`.
  * Periksa state akun: saldo dan nonce dari pengirim Transaksi #1 sampai #4 wajib tetap utuh seperti sebelum blok dieksekusi (tidak ada mutasi setengah jalan / *no partial mutation*).

### [ST2] Konservasi Saldo & Proteksi Aritmatika Nir-Pecahan (Solvency & Strict Quanta Conservation)
* **Deskripsi:** Total Quanta dalam sistem tidak boleh tercipta atau musnah dari ketiadaan, kecuali melalui mekanisme protokol yang sah (genesis/minting terjadwal atau burn fee). Mutasi saldo dilarang keras mengalami *underflow* atau *overflow*.
* **Kriteria Uji:**
  * Uji transfer saldo: pengurangan saldo pengirim wajib tepat sama dengan pertambahan saldo penerima ditambah fee:
    $$\Delta \text{Balance}_{\text{sender}} = \text{Amount} + \text{Fee}$$
    $$\Delta \text{Balance}_{\text{recipient}} = \text{Amount}$$
  * Percobaan pengurangan saldo melebihi saldo yang tersedia wajib menghasilkan error `InsufficientBalance` tanpa memicu panic atau integer wrap.
  * Uji boundary: penambahan saldo mendekati `u128::MAX` wajib dilindungi oleh `checked_add` dan menolak overflow secara anggun.

### [ST3] Monotonisitas Nonce per Akun (Strict Nonce Monotonicity)
* **Deskripsi:** Setiap transaksi yang berhasil dieksekusi wajib menaikkan nonce akun pengirim tepat sebesar 1 ($N_{\text{new}} = N_{\text{current}} + 1$).
* **Kriteria Uji:**
  * Eksekusi transaksi dengan nonce yang cocok ($N = N_{\text{state}}$) $\to$ transaksi berhasil, nonce akun di state berubah menjadi $N+1$.
  * Percobaan eksekusi transaksi dengan nonce masa lalu ($N < N_{\text{state}}$) wajib ditolak dengan `Err(StateTransitionError::InvalidNonce)`.
  * Percobaan eksekusi transaksi dengan nonce masa depan yang melompat ($N > N_{\text{state}}$) pada blok aktif wajib ditolak (karena transaksi harus berurutan).
  * Transaksi yang gagal validasi di awal tidak boleh menaikkan nonce akun.

### [ST4] Akuntansi Fee Transaksi & Alokasi Pembakaran/Validator (Fee Accounting & Deduction)
* **Deskripsi:** Pemotongan fee transaksi wajib dihitung secara deterministik dan dicatat ke akun penampung validator (*coinbase/fee collector*) atau dibakar (*burn*) sesuai aturan protokol tanpa manipulasi float.
* **Kriteria Uji:**
  * Pengirim membayar fee $F$ Quanta.
  * Saldo pengirim berkurang sebesar $F$.
  * Alokasi fee ke reward pool atau alamat validator meningkat tepat sebesar $F$ (atau proporsi pembagian yang dihitung murni dengan pembagian integer dan sisa bagi yang terverifikasi).
  * Transaksi dengan fee 0 pada kondisi jaringan yang mewajibkan minimum fee wajib ditolak.

### [ST5] Integritas Struktural Blok & Tautan Rantai (Block Header Integrity & Gapless Linkage)
* **Deskripsi:** Blok baru hanya valid jika mempertahankan silsilah rantai yang sah: tinggi blok bertambah tepat 1 ($H_n = H_{n-1} + 1$), `parent_hash` merujuk tepat ke hash blok sebelumnya, dan `state_root` pada header cocok dengan hasil komputasi state pasca-eksekusi.
* **Kriteria Uji:**
  * Blok dengan `parent_hash` yang salah/berbeda wajib ditolak dengan `Err(StateTransitionError::InvalidParentHash)`.
  * Blok dengan ketinggian melompat ($H+2$) atau mundur ($H-1$) wajib ditolak dengan `Err(StateTransitionError::NonSequentialHeight)`.
  * Blok dengan timestamp yang lebih tua dari blok sebelumnya atau melompat jauh ke masa depan (melebihi ambang batas toleransi) wajib ditolak.
  * Blok yang mencantumkan `state_root` palsu (tidak cocok dengan hasil eksekusi transaksi) wajib ditolak dengan `Err(StateTransitionError::StateRootMismatch)`.

---

## 2.6 Fakta kanonikal `BlockHeader` (sinkron protokol 2026-09-30)

Header blok **persis 6 field**; `chain_id`, `round`, maupun `qc` tidak pernah
menjadi bagian `BlockHeader` (rantai diidentifikasi oleh tautan `prev_hash`,
bukan label id):

| Field | Tipe | Keterangan |
| --- | --- | --- |
| `height` | `u64` | Tinggi blok; genesis `H = 0` |
| `prev_hash` | `Hash256` | Hash blok induk (`[0u8; 32]` untuk genesis) |
| `state_root` | `Hash256` | Komitmen BLAKE3 state pasca-eksekusi |
| `tx_count` | `u32` | Jumlah transaksi dalam blok (**bukan** `u64`) |
| `timestamp` | `u64` | Milidetik Unix Epoch (UTC) |
| `proposer` | `[u8; 32]` | Penerima fee blok |

`Block = { header, transactions }`; header hash domain `AURION_BLOCK_HEADER_V1`
mencakup `timestamp || proposer` (ST5). `CreateBlock`/genesis sintetis wajib
memakai bentuk ini — lihat juga `TASK-aurion-genesis.md` §5.1.

---

## 3. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian modul ini dikonsolidasikan sebagai suite integrasi internal modul `aurion-core`:

* **Target Modul:** `crates/aurion-core/`
* **Lokasi Suite Uji:** `crates/aurion-core/tests/core_invariants.rs`
* **Dependensi Pengujian (Dev-Dependencies):**
  * `aurion-criptografi`: Pembangkitan kunci Ed25519 dan verifikasi tanda tangan transaksi uji.

---

## 4. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-core` dinyatakan selesai apabila:
1. Berkas spesifikasi tersimpan di `docs/task-register/TASK-aurion-core.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji ST0–ST5 diimplementasikan di `crates/aurion-core/tests/core_invariants.rs` dan seluruhnya lulus:
   ```bash
   cargo test -p aurion-core --test core_invariants

```

3. Seluruh unit test inline di dalam crate tetap lulus:
```bash
cargo test -p aurion-core

```


4. Lolos audit kepatuhan statis tanpa error dan warning:
```bash
cargo clippy -p aurion-core --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check

```


5. Ringkasan temuan dan konfirmasi status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.

```

---

### Instruksi untuk Diberikan ke Agen

Salin teks berikut untuk memerintahkan agen memulai eksekusi:

> "Daftarkan spesifikasi pengujian di atas ke `docs/task-register/TASK-aurion-core.md`, perbarui indeks di `docs/STRUKTUR-FOLDER.txt`, lalu bangun suite pengujian integrasi `crates/aurion-core/tests/core_invariants.rs` yang menguji secara komprehensif seluruh matriks invarian **ST0 sampai ST5** (Determinisme State Root BLAKE3, Atomisitas Eksekusi Blok, Konservasi Saldo Quanta, Monotonisitas Nonce, Akuntansi Fee, dan Integritas Struktural Blok/Header)."

<FollowUp label="Mau kita minta agen mengimplementasikan file pengujian core_invariants.rs sekarang?" query="Perintahkan agen untuk mendaftarkan docs/task-register/TASK-aurion-core.md dan mengimplementasikan suite pengujian core_invariants.rs pada crates/aurion-core sesuai matriks ST0-ST5."/>

```

## Keputusan protokol resmi ST4/ST5 (2026-09-29)

- **ST4 fee:** `Transaction.fee: Quanta` (`u128`) adalah bagian integral payload yang ditandatangani. Hash tanda tangan `BLAKE3(nonce || sender || recipient || amount || fee)` dengan domain `AURION_TX_CANONICAL_V1`, sehingga manipulasi fee membatalkan tanda tangan (`InvalidSignature`).
- **Akuntansi ST4:** outflow `amount.checked_add(fee)`, solvency `sender.balance >= amount + fee`; `sender -= amount + fee`, `recipient += amount`, `fee` dikreditkan ke `BlockHeader.proposer` (fallback `PROTOCOL_FEE_SINK = [0x00; 32]`). `fee == 0` ditolak (`FeeTooLow`).
- **ST5 timestamp:** `BlockHeader.timestamp: u64` milidetik Unix Epoch (UTC) + `proposer: [u8; 32]` penerima fee; header hash mencakup `timestamp || proposer` di bawah domain `AURION_BLOCK_HEADER_V1`.
- **Validasi deterministik `aurion-core`:** `T_block > T_parent`, genesis `H=0` sebagai acuan; pelanggaran `Err(TimestampNotMonotonic)`. Batas 5000 ms future drift adalah kebijakan `aurion-consensus` saat voting (non-deterministik jam dinding), bukan replay `aurion-core`.

## Status implementasi dan batas protokol saat ini

- ST0–ST5 terimplementasi dan diuji: ST0 determinisme/replay 50 transaksi; ST1 rollback atomik; ST2 solvency/underflow/overflow; ST3 nonce; ST4 fee bertanda tangan, solvency `amount+fee`, kredit proposer/sink, dan penolakan fee nol/tamper; ST5 sequential height, parent hash, `T_block > T_parent`, tx count, dan state root via `Block::execute_with_parent`.
- Batas lanjutan: `MAX_FUTURE_DRIFT_MS = 5000` belum diimplementasikan di `aurion-consensus` (validasi jam dinding non-deterministik); belum ada `MIN_FEE` konfigurable di core (`fee > 0` saat ini); sink `[0x00; 32]` belum memiliki kebijakan burn/withdraw terjadwal.
