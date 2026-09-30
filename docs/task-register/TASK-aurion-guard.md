# TASK: aurion-guard
> **Target Modul:** `aurion-guard` (`crates/aurion-guard`)  
> **Kategori:** Mesin Penegakan Keamanan, Verifikasi Bukti Pelanggaran & Slashing (Security & Slashing Engine)  
> **Status:** COMPLETED
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper)  
> **Tingkat Ketergantungan:** 3 (`aurion-core`, `aurion-criptografi`, `aurion-account`, `aurion-consensus`, `aurion-validator`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-guard` adalah benteng pertahanan integritas dan penegak sanksi ekonomi protokol Aurion. Modul ini bertanggung jawab memproses bukti kecurangan kriptografis (*equivocation evidence* yang dihasilkan oleh subsistem konsensus B2), mengeksekusi penalti pemotongan jaminan (*slashing*), memelihara daftar cekal validator Byzantin (*tombstoning/jailing*), mendistribusikan insentif pelapor (*whistleblower reward*), dan memvalidasi tindakan darurat Dewan Pengawas (*GuardCouncil*).

Sebagai modul penegak hukum ekonomi, kegagalan pada `aurion-guard` dapat melumpuhkan seluruh rantai: *false positive slashing* dapat merugikan validator jujur secara tidak adil, *double slashing replay* dapat menguras habis jaminan melebihi batas yang sah, sedangkan kegagalan eksekusi sanksi memungkinkan simpul jahat melakukan serangan *forking* tanpa konsekuensi finansial.

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-guard` secara terisolasi** agar:
1. Memverifikasi keabsahan kriptografis bukti pelanggaran konsensus (*Equivocation Evidence Verification*) sebelum sanksi dipicu.
2. Mencegah pemutaran ulang bukti (*anti-replay*) dan menolak bukti kedaluwarsa di luar jendela audit (*unbonding window*).
3. Menerapkan sanksi pemotongan jaminan (*slashing*) bertingkat secara deterministik dengan pembagian hasil denda (*burn*, *reporter reward*, dan *treasury*) yang mempertahankan konservasi total Quanta.
4. Menegakkan pengusiran seketika (*instant quorum eviction*) bagi validator yang terkena sanksi berat melalui status `Tombstoned` atau penahanan sementara via `Jailed`.
5. Mengatur kewenangan hak istimewa Dewan Pengawas (*GuardCouncil*) dengan validasi kuorum multi-signature murni tanpa celah tindakan sepihak (*unilateral abuse*).
6. Mengunci seluruh perhitungan tarif penalti, denda, dan insentif pelapor murni pada aritmatika integer basis poin (BPS, $10.000 = 100%$) bertipe `u128` Quanta (rasio BPS `u64`) (*zero-float*).
7. Menegakkan kepatuhan arsitektur: `#![forbid(unsafe_code)]`, zero-float, penolakan `.unwrap()` di jalur produksi, serta penanganan error bertipe kuat melalui `thiserror`.

---

## 2. ALUR EKSEKUSI BUKTI KECURANGAN & PENALTI

```text
               [Laporan Bukti Pelanggaran Konsensus (B2)]
                                   |
                                   v
                 +---------------------------------------+
                 | [G0] Verifikasi Bukti Kripto      |
                 | - 2 Tanda tangan Ed25519 valid    |
                 | - Tinggi & Ronde sama, Hash beda  |
                 | - Validator terdaftar di epoch    |
                 +---------------------------------------+
                                   |
                                   v Sah
                 +---------------------------------------+
                 | [G1] Guard Kedaluwarsa & Replay   |
                 | - Cek usia blok <= MAX_EVIDENCE_AGE|
                 | - Bukti belum pernah dieksekusi   |
                 +---------------------------------------+
                                   |
                                   v Belum pernah diproses
                 +---------------------------------------+
                 | [G2/G5] Kalkulasi Denda BPS (u128) |
                 | - Hitung Severe Slash (misal 30%) |
                 | - Hitung Hadiah Pelapor (Reporter)|
                 | - Alokasikan Burn & Fee Sink      |
                 +---------------------------------------+
                                   |
                                   v
                 +---------------------------------------+
                 | [G3] Mutasi Status & Isolasi      |
                 | - Kurangi Stake Validator         |
                 | - Set Status: Tombstoned / Jailed |
                 | - Cabut Hak Voting Konsensus      |
                 +---------------------------------------+
                                   |
                                   v
                 [Write-Set Diterapkan ke State Utama]
```

---

## 3. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (G0 - G5)

Suite pengujian modul wajib menguji 6 dimensi invarian keamanan dan penalti berikut:

### [G0] Verifikasi Bukti Kriptografis Equivocation (Equivocation Evidence Cryptographic Verification)

**Deskripsi:** Bukti *equivocation* (suara ganda atau proposal ganda oleh validator yang sama pada tinggi dan ronde konsensus yang identik) wajib divalidasi tanda tangan kriptografisnya dan dibuktikan menghasilkan hash komitmen yang saling bertentangan.

**Kriteria Uji:**
- Bukti valid: Dua suara BFT dengan signer yang sama, target `height` dan `round` yang sama, namun `block_hash_a != block_hash_b`, dengan kedua tanda tangan Ed25519 sah -> wajib diterima (`Ok(VerifiedEvidence)`).
- Bukti palsu dengan salah satu tanda tangan Ed25519 korup/salah wajib ditolak seketika dengan `Err(GuardError::InvalidEvidenceSignature)`.
- Bukti dengan hash blok yang identik (`block_hash_a == block_hash_b`) wajib ditolak karena bukan pelanggaran (*non-conflicting votes*) dengan `Err(GuardError::NonConflictingEvidence)`.
- Bukti dengan tinggi blok atau ronde yang berbeda wajib ditolak dengan `Err(GuardError::MismatchedHeightOrRound)`.
- Bukti yang mencantumkan kunci publik yang tidak terdaftar sebagai validator pada epoch tersebut wajib ditolak dengan `Err(GuardError::UnknownValidator)`.

### [G1] Proteksi Replay & Kedaluwarsa Bukti (Evidence Expiry & Anti-Replay Protection)

**Deskripsi:** Setiap bukti pelanggaran yang sah hanya boleh dieksekusi satu kali. Bukti yang diajukan melampaui jendela audit (*slashing window / unbonding period*) wajib ditolak untuk menjaga stabilitas finalitas historis.

**Kriteria Uji:**
- Bukti yang telah berhasil dieksekusi pada blok H dicatat digest-nya ke dalam `ExecutedEvidenceLedger`.
- Pengajuan ulang bukti yang sama persis (atau variasi urutan vote B <-> A) wajib ditolak dengan `Err(GuardError::EvidenceAlreadyExecuted)`.
- Penolakan bukti kedaluwarsa: Jika bukti berasal dari tinggi blok H_evidence dan tinggi blok saat ini H_current > H_evidence + MAX_EVIDENCE_AGE_BLOCKS (misal: 10.000 blok), bukti wajib ditolak dengan `Err(GuardError::EvidenceExpired)`.
- Upaya memicu eksekusi ganda dalam satu batch transaksi yang sama wajib digagalkan secara atomik.

### [G2] Eksekusi Slashing Bertingkat & Konservasi Solvensi (Tiered Slashing & Fine Solvency)

**Deskripsi:** Penalti pemotongan stake dibedakan berdasarkan tingkat keparahan pelanggaran: Pelanggaran Berat (*Equivocation/Double-Signing*) dan Pelanggaran Ringan (*Liveness Fault*). Distribusi potongan denda wajib mempertahankan prinsip konservasi total Quanta.

**Kriteria Uji:**
- Pelanggaran Berat (*Severe Slashing*): Memotong jaminan sebesar tarif denda berat (`SEVERE_SLASH_BPS`, misal: 3.000 BPS / 30%).
- Pembagian hasil denda:
  
  Slash Quanta = Reward_reporter + Quanta_burned + Quanta_sink
  
  Dihitung murni dengan integer; total debit pada akun validator wajib sama persis dengan total kredit pelapor ditambah pembakaran dan setoran ke penampung protokol (*zero leakage*).
- Jika sisa saldo stake validator pasca-denda jatuh di bawah batas cadangan minimal akun, sisa debu (*dust*) dialokasikan ke sink protokol secara teratur tanpa memicu underflow.

### [G3] Isolasi Status: Tombstoning Permanen & Auto-Jail (Instant Eviction & Tombstoning)

**Deskripsi:** Validator yang terbukti melakukan pelanggaran berat wajib di-tombstone secara permanen dan dikeluarkan seketika dari penghitungan kuorum aktif konsensus tanpa hak pemulihan.

**Kriteria Uji:**
- Eksekusi severe slash mengubah status validator menjadi `ValidatorStatus::Tombstoned`.
- Validator berstatus `Tombstoned` dicabut seluruh hak suaranya; bobot kuorum konsensus pada blok berikutnya otomatis berkurang sebesar bobot validator tersebut.
- Kunci publik konsensus validator yang di-tombstone dimasukkan ke dalam daftar cekal permanen (*TombstoneBlacklist*). Pendaftaran ulang menggunakan kunci yang sama di masa depan wajib ditolak.
- Pelanggaran ringan (*Liveness Fault*) mengubah status menjadi `ValidatorStatus::Jailed` (dikeluarkan sementara, berhak mengajukan unjail setelah masa pendinginan berakhir).

### [G4] Otorisasi & Kuorum Dewan Pengawas (GuardCouncil Governance & Action Authorization)

**Deskripsi:** Tindakan luar biasa (pembekuan simpul darurat, pembatalan status darurat, atau penyesuaian parameter denda) hanya dapat disahkan melalui persetujuan multi-sig resmi dari `GuardCouncil`.

**Kriteria Uji:**
- Aksi darurat `EmergencyAction` membutuhkan tanda tangan sah dari anggota `GuardCouncil` yang memenuhi ambang batas kuorum (misal: >= 67%).
- Permohonan aksi darurat yang diajukan oleh akun dengan peran `StandardUser` atau `ActiveValidator` biasa wajib ditolak dengan `Err(GuardError::UnauthorizedCouncilAction)`.
- Tanda tangan anggota council yang tidak terdaftar atau telah dicabut mandatnya wajib ditolak dengan `Err(GuardError::InvalidCouncilMember)`.
- Tanda tangan multi-sig dewan pengawas terikat pada `chain_id` dan `council_nonce` untuk mencegah serangan replay lintas rantai atau antar-proposal.

### [G5] Akuntansi Denda Nir-Pecahan Berbasis BPS (Zero-Float BPS Slash Accounting)

**Deskripsi:** Seluruh penghitungan denda pemotongan jaminan, alokasi hadiah pelapor, dan rasio kuorum dewan pengawas wajib menggunakan bilangan bulat murni (`u128` Quanta) dengan presisi Basis Poin (10.000 BPS = 100%).

**Kriteria Uji:**
- Perhitungan pemotongan denda:
  
  Slash Quanta = (staked_quanta * slash_rate_bps) / 10.000
  
  dieksekusi menggunakan `checked_mul` diikuti `checked_div` dengan pembulatan ke bawah (*floor division*).
- Perhitungan insentif pelapor:
  
  Reporter Reward = (Slash Quanta * reporter_reward_bps) / 10.000
  
  dijamin tidak melebihi total denda yang dipotong.
- Semua operasi aritmatika dilindungi dari potensi *overflow* dan *underflow*.
- Audit statis: Tidak ada satupun token `f32` atau `f64` pada seluruh berkas sumber di dalam `crates/aurion-guard/src/`.

---

## 4. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian modul ini dikonsolidasikan sebagai suite integrasi internal modul `aurion-guard`:

- **Target Modul:** `crates/aurion-guard/`
- **Lokasi Suite Uji:** `crates/aurion-guard/tests/guard_invariants.rs`
- **Dependensi Pengujian (Dev-Dependencies):**
  - `aurion-core`: Tipe data `AccountId`, `Quanta`, `BlockHeader`.
  - `aurion-criptografi`: Pembangkitan kunci Ed25519 dan pembuatan bukti tanda tangan palsu/asli.
  - `aurion-consensus`: Struktur `VoteMessage` dan `EquivocationProof`.
  - `aurion-validator`: Struktur `ValidatorStatus` dan transisi `Jailed`/`Tombstoned`.
  - `aurion-account`: Struktur peran `Role::GuardCouncil`.

---

## 5. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-guard` dinyatakan selesai apabila:

1. Berkas spesifikasi tersimpan di `docs/task-register/TASK-aurion-guard.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji G0-G5 diimplementasikan di `crates/aurion-guard/tests/guard_invariants.rs` dan seluruhnya lulus:
```bash
cargo test -p aurion-guard --test guard_invariants
```

3. Seluruh unit test inline di dalam crate tetap lulus:
```bash
cargo test -p aurion-guard
```

4. Lolos audit kepatuhan statis tanpa error dan warning:
```bash
cargo clippy -p aurion-guard --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check
```

5. Tidak ada penggunaan `unsafe`, tipe `f32`/`f64`, atau `.unwrap()` di jalur produksi `crates/aurion-guard/src/`.
6. Ringkasan temuan dan konfirmasi status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.

---

### Instruksi Penyimpanan & Delegasi Agen

Daftarkan berkas spesifikasi di atas ke dalam repositori lokal dengan perintah:

```bash
cat << 'EOF' > /home/ratu/workspace/aurion/docs/task-register/TASK-aurion-guard.md
# (Tempelkan seluruh isi dokumen spesifikasi di atas)
EOF
```

Atau berikan instruksi langsung kepada agen pengerjaan:

> "Daftarkan spesifikasi pengujian di atas ke `docs/task-register/TASK-aurion-guard.md`, perbarui indeks di `docs/STRUKTUR-FOLDER.txt`, lalu bangun modul `crates/aurion-guard` dan suite pengujian integrasi `crates/aurion-guard/tests/guard_invariants.rs` yang menguji secara komprehensif seluruh matriks invarian **G0 sampai G5** (Verifikasi Kripto Equivocation, Anti-Replay & Kedaluwarsa Bukti, Slashing Bertingkat Solven, Tombstoning Permanen, Kuorum Multi-Sig GuardCouncil, dan Akuntansi Denda BPS Zero-Float)."
