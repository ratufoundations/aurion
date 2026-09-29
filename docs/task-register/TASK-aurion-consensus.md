```markdown
# TASK: task-aurion-consensus
> **Target Modul:** `aurion-consensus` (`crates/aurion-consensus`)  
> **Kategori:** Modul Konsensus BFT & SMR Coordinator (Consensus Core)  
> **Status:** COMPLETED  
> **Otoritas:** `Agents.md`  
> **Tingkat Ketergantungan:** 2 (`aurion-core`, `aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-consensus` adalah koordinator utama replikasi mesin keadaan (*State Machine Replication / SMR*). Modul ini bertanggung jawab memimpin validator dalam menyepakati urutan blok transaksi secara deterministik, memvalidasi tanda tangan suara (*votes*), membentuk sertifikat kuorum (*Quorum Certificate / QC*), mendeteksi pelanggaran protokol (*equivocation / double-voting*), serta menjamin *safety* (tidak terjadi *fork*) dan *liveness* (rantai tidak macet saat terjadi kegagalan atau timeout simpul).

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-consensus` secara terisolasi** agar:
1. Memverifikasi keabsahan kriptografis setiap suara (*vote*) terhadap himpunan validator aktif (*ValidatorSet*).
2. Menegakkan pembentukan Quorum Certificate (QC) secara ketat pada ambang batas super-mayoritas $2f + 1$.
3. Mendeteksi dan menolak suara ganda (*equivocation*) dari validator jahat serta membangkitkan bukti pelanggaran (*slashing evidence*).
4. Menjaga monotonisitas pergantian ronde (*round progression*) dan menepis suara usang (*stale votes*) tanpa mencemari ronde aktif.
5. Menjamin *liveness* sistem melalui mekanisme timeout dan pergantian pemimpin (*view-change*) yang deterministik.
6. Menghitung seluruh batas kuorum dan bobot voting murni menggunakan aritmatika integer (`u64`) tanpa float (*zero-float*).
7. Mematuhi invarian protokol: `#![forbid(unsafe_code)]`, zero-float, zero-unwrap di jalur produksi, dan penanganan galat terstruktur via `thiserror`.

---

## 2. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (B0 – B5)

Suite pengujian modul wajib memvalidasi 6 dimensi invarian konsensus BFT berikut:

### [B0] Validasi Kriptografi Suara & Keanggotaan Validator (Vote Signature & Membership Verification)
* **Deskripsi:** Setiap suara proposal blok (`Vote`) wajib ditandatangani oleh validator sah yang terdaftar dalam `ValidatorSet` untuk ketinggian (*height*) dan ronde (*round*) yang relevan.
* **Kriteria Uji:**
  * Suara dari validator terdaftar dengan tanda tangan Ed25519 valid wajib diterima (`Ok(())`).
  * Suara dengan tanda tangan rusak atau payload yang dimodifikasi 1 bit wajib ditolak seketika dengan `Err(ConsensusError::InvalidVoteSignature)`.
  * Suara yang ditandatangani oleh entitas di luar `ValidatorSet` wajib ditolak dengan `Err(ConsensusError::UnknownValidator)`.
  * Penolakan suara cacat tidak boleh mengubah state akumulator suara internal.

### [B1] Ambang Batas Kuorum Super-Mayoritas ($2f + 1$ Quorum Certificate)
* **Deskripsi:** Quorum Certificate (QC) hanya boleh diterbitkan jika akumulasi bobot suara sah mencapai super-mayoritas:
  $$Q \ge 2f + 1 \quad \text{di mana} \quad f = \left\lfloor \frac{N - 1}{3} \right\rfloor$$
  atau $\ge 67\%$ dari total bobot voting pada model *weighted-voting*.
* **Kriteria Uji:**
  * Untuk kluster $N = 4$ ($f = 1$): kumpulkan 2 suara $\to$ QC belum terbentuk; begitu suara ke-3 ($2f + 1 = 3$) masuk, QC wajib berhasil dibuat.
  * Suara duplikat dari validator yang sama tidak boleh dihitung dua kali dalam akumulasi kuorum.
  * QC yang dihasilkan wajib memuat referensi hash blok, height, round, dan agregasi tanda tangan/bitmask pemilih yang terverifikasi.

### [B2] Deteksi Suara Ganda & Mitigasi Equivocation (Double-Vote Guard & Equivocation Evidence)
* **Deskripsi:** Validator dilarang menandatangani dua proposal blok yang berbeda pada height, round, dan fase voting yang sama (*double-voting/equivocation*).
* **Kriteria Uji:**
  * Validator $V_i$ mengirim suara untuk Blok $A$ pada $(H=1, R=0)$. Suara pertama diterima.
  * Validator $V_i$ yang sama mengirim suara untuk Blok $B$ ($A \ne B$) pada $(H=1, R=0)$.
  * Modul wajib menolak suara kedua dengan `Err(ConsensusError::EquivocationDetected(validator))` dan memproduksi struct `EquivocationEvidence` yang memuat kedua suara bertanda tangan sah sebagai bukti pelanggaran yang tidak terbantahkan.

### [B3] Progresi Ronde & Penolakan Suara Usang (Monotonic Round Progression & Stale Vote Filter)
* **Deskripsi:** Status konsensus bergerak maju secara monotonik. Suara dari masa lalu atau masa depan yang terlalu jauh tidak boleh merusak state aktif.
* **Kriteria Uji:**
  * Jika ronde aktif saat ini adalah $R=3$:
    * Suara yang masuk untuk ronde lampau ($R < 3$) atau ketinggian lampau ($H < \text{current}$) wajib diabaikan secara anggun (*silently dropped*) atau ditolak dengan `Err(ConsensusError::StaleVote)` tanpa memicu panic.
    * Suara untuk ronde aktif ($R=3$) diproses normal.
    * Transisi ronde dari $R$ ke $R+1$ wajib mereset akumulator suara ronde sebelumnya dan mengunci status blok ronde sebelumnya.

### [B4] Mekanisme Timeout & Pergantian Leader Deterministik (Pacemaker & View-Change Liveness)
* **Deskripsi:** Ketika leader tidak mengusulkan blok hingga batas waktu berakhir, validator harus dapat menyepakati pergantian ronde (*view-change*) tanpa terjadi kebuntuan (*deadlock*).
* **Kriteria Uji:**
  * Simulasikan peristiwa timeout ronde pada $(H=1, R=0)$.
  * Validator menerbitkan pesan bertanda tangan `TimeoutVote`.
  * Ketika kuorum timeout tercapai ($2f + 1$ timeout votes), ronde berganti secara deterministik ke $R=1$.
  * Pemilihan leader baru untuk ronde $R=1$ wajib deterministik (misal: algoritma *round-robin* berbasis index validator):
    $$\text{Leader Index} = (H + R) \pmod N$$
    Semua validator harus menunjuk leader yang sama persis tanpa koordinasi eksternal.

### [B5] Aritmatika Kuorum Nir-Pecahan (Zero-Float Integer Quorum Arithmetic)
* **Deskripsi:** Seluruh kalkulasi batas toleransi kesalahan byzantine ($f$), ambang kuorum ($2f + 1$), dan akumulasi bobot voting wajib menggunakan integer murni (`u64`) dengan proteksi *checked arithmetic*.
* **Kriteria Uji:**
  * Uji fungsi ambang kuorum untuk berbagai ukuran kluster validator:
    * $N = 1 \implies f = 0, \text{Quorum} = 1$
    * $N = 3 \implies f = 0, \text{Quorum} = 1$ (atau $2$ sesuai spesifikasi parsial)
    * $N = 4 \implies f = 1, \text{Quorum} = 3$
    * $N = 7 \implies f = 2, \text{Quorum} = 5$
    * $N = 10 \implies f = 3, \text{Quorum} = 7$
    * $N = 100 \implies f = 33, \text{Quorum} = 67$
  * Pastikan tidak ada representasi tipe `f32` atau `f64` di seluruh modul, dan operasi penambahan bobot voting menggunakan `checked_add` untuk mencegah *overflow attack*.

---

## 3. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian modul ini dikonsolidasikan sebagai suite integrasi internal modul `aurion-consensus`:

* **Target Modul:** `crates/aurion-consensus/`
* **Lokasi Suite Uji:** `crates/aurion-consensus/tests/consensus_invariants.rs`
* **Dependensi Pengujian (Dev-Dependencies):**
  * `aurion-core`: Struktur data `Block`, `BlockHeader`, dan hash identifier.
  * `aurion-criptografi`: Pembangkitan pasangan kunci Ed25519 validator dan fungsi penandatanganan suara.

---

## 4. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-consensus` dinyatakan selesai apabila:
1. Berkas spesifikasi tersimpan di `docs/task-register/TASK-aurion-consensus.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji B0–B5 diimplementasikan di `crates/aurion-consensus/tests/consensus_invariants.rs` dan seluruhnya lulus:
   ```bash
   cargo test -p aurion-consensus --test consensus_invariants

```

3. Seluruh unit test inline di dalam crate tetap lulus:
```bash
cargo test -p aurion-consensus

```


4. Lolos audit kepatuhan statis tanpa error dan warning:
```bash
cargo clippy -p aurion-consensus --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check

```


5. Ringkasan temuan dan konfirmasi status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.

```

---

### Instruksi untuk Diberikan ke Agen

Salin teks berikut untuk memerintahkan agen pengerjaan:

> "Daftarkan spesifikasi pengujian di atas ke `docs/task-register/TASK-aurion-consensus.md`, perbarui indeks di `docs/STRUKTUR-FOLDER.txt`, lalu bangun suite pengujian integrasi `crates/aurion-consensus/tests/consensus_invariants.rs` yang menguji secara komprehensif seluruh matriks invarian **B0 sampai B5** (Validitas Suara Kriptografis, Ambang Kuorum $2f+1$, Proteksi Double-Vote & Equivocation, Progresi Ronde Deterministik, Timeout/View-Change Pacemaker, dan Aritmatika Kuorum Zero-Float)."

<FollowUp label="Mau kita minta agen mengimplementasikan file pengujian consensus_invariants.rs sekarang?" query="Perintahkan agen untuk mendaftarkan docs/task-register/TASK-aurion-consensus.md dan membangun suite pengujian consensus_invariants.rs pada crates/aurion-consensus sesuai matriks B0-B5."/>

```

## Catatan implementasi dan batas cakupan

- `ValidatorSet` saat ini memakai model satu validator satu suara (bobot implisit 1), bukan bobot voting yang dapat dikonfigurasi. Ambang dihitung dengan `checked_mul`/`checked_add`; API mengembalikan `ArithmeticOverflow` bila hasil tidak dapat direpresentasikan. Contoh `N=3` mengikuti formula literal task: `f=0`, kuorum `1`.
- `RoundState` menyediakan akumulasi timeout bertanda tangan dan perpindahan ronde ketika kuorum tercapai. Pengukuran waktu/pacemaker wall-clock tetap menjadi tanggung jawab pemanggil; test suite mensimulasikan timeout dengan mengirim timeout vote.
- Lock safety yang tersedia menyimpan QC prevote terakhir saat round berubah dan mereset accumulator vote. Protokol belum memiliki aturan proposal/unlock lintas round atau transport multi-node; suite menguji koordinator lokal yang tersedia.
- Error aktual memakai `InvalidVoteSignature`, `UnknownValidator(pubkey)`, `StaleVote`, dan `EquivocationDetected(pubkey)` agar detail tipe tetap eksplisit.
