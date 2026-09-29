# TASK: task-aurion-validator
> **Target Modul:** `aurion-validator` (`crates/aurion-validator`)
> **Kategori:** Modul Siklus Hidup Validator (Admission, Probation, Rotasi Epoch, Liveness, Endorsement, Skoring BPS)
> **Status:** COMPLETED (2026-09-29) — suite V0–V5 hijau (19 tes), audit statis lolos
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper)
> **Tingkat Ketergantungan:** 2 (`aurion-criptografi`, `aurion-account`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-validator` mengatur gerbang penerimaan validator (bukti kepemilikan kunci konsensus *PoP*, ambang stake, peran akun), pipeline masa percobaan (*probation*), rotasi himpunan validator aktif yang deterministik per epoch, penegakan liveness dengan penahanan otomatis (*auto-jail*), kuota endorsement anti-Sybil, serta akuntansi stake berbasis basis poin (*BPS*) tanpa pecahan.

Kegagalan pada modul ini dapat berakibat fatal: masuknya validator palsu (*Sybil*), himpunan aktif yang kosong/osilasi akibat rotasi, validator yang tidak pernah pulih dari penahanan (*bricked*), atau pembengkakan bobot kuorum akibat pembulatan pecahan.

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-validator` secara terisolasi** agar:
1. Gerbang penerimaan menolak peran salah, stake di bawah ambang, PoP silang/dipalsukan, dan duplikasi kunci konsensus — seluruhnya atomik.
2. Jendela probation hanya bertambah pada blok bertanda tangan, direset oleh pelanggaran liveness, dan membatalkan kandidat berulang ke `Retired`.
3. Rotasi epoch hanya pada batas epoch, deterministik byte-per-byte, terurut skor dengan pemutus seri `AccountId`, dan incumbent tetap terpilih (tanpa osilasi himpunan).
4. Validator aktif ditahan otomatis tepat pada ambang blok terlewat, keluar dari kuorum seketika, serta pemulihan wajib melewati cooldown dan otorisasi master anti-replay.
5. Kuota endorsement, masa kerja pengesah, target kandidat, bobot anti-Sybil (pasangan timbal balik dinetralkan), serta penodaan (*taint*) pasca-slashing berat ditegakkan.
6. Seluruh aritmetika stake/kuorum/slash memakai bilangan bulat `u64` dengan pembagian lantai dan galat bertipe (*zero-float*).
7. Menegakkan disiplin kode: `#![forbid(unsafe_code)]`, zero-float, penolakan `.unwrap()` di jalur produksi, serta error bertipe kuat melalui `thiserror`.

---

## 2. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (V0-V5)

### [V0] Gerbang Penerimaan: Peran, Ambang Stake, dan PoP
* Pendaftaran wajib peran `ValidatorCandidate`, stake >= `MIN_VALIDATOR_STAKE_QUANTA`, `ProofOfPossession` terikat akun, dan kunci konsensus unik; seluruh penolakan atomik (`state_digest` tidak berubah).
* Peran salah -> `UnauthorizedRole { expected, actual }`; stake kurang -> `InsufficientStake { provided, required }`; PoP silang/dipalsukan -> `InvalidProofOfPossession`; kunci ganda -> `DuplicateConsensusKey`.
* Kebijakan bawaan selaras konstanta protokol; ambang nol -> `InvalidValidatorPolicy`.

### [V1] Pipeline Probation & Matriks Status
* Jendela `K` blok: `K-1` blok sah belum lulus, blok ke-K meluluskan ke `Eligible`; `Probation` tidak ikut kuorum, seleksi, maupun aktivasi paksa.
* Pelanggaran liveness me-reset jendela ke nol; `probation_max_miss_streak` pelanggaran berurutan -> `ProbationFailed` dan status `Retired` (terminal, pelaporan lanjutan ditolak).
* Matriks transisi persis 14 pasangan sah; `authorize_transition` selalu sinkron dengan `can_transition_to`; `Probation` tidak pernah boleh melompat ke `ActiveSet`.

### [V2] Rotasi Epoch Deterministik
* Hanya batas epoch (`height % epoch_length_blocks == 0`; panjang nol fail-closed) — selain itu `EpochNotBoundary` tanpa mutasi state.
* Dua mesin dengan urutan pendaftaran berbeda menghasilkan `state_digest` dan seleksi identik; peringkat: skor BPS menurun -> stake mentah menaik -> `AccountId` menaik; kapasitas memangkas kandidat bawah.
* Incumbent `ActiveSet` tetap terpilih pada rotasi berikutnya (tanpa osilasi); penurunan kursi menuju `Eligible` (bukan `Jailed`) dengan `active_since_block` tersalin tepat.

### [V3] Liveness: Auto-Jail, Cooldown Unjail, Penangguhan
* `max_missed_blocks` blok terlewat berurutan -> `AutoJailed` tepat pada ambang, keluar dari kuorum seketika, `jail_start_block` tercatat.
* `unjail` wajib cooldown penuh, kunci master terdaftar, dan nonce cocok (anti-replay); nonce maju atomik setelah sukses sehingga tanda tangan lama basi.
* Penangguhan dewan/tata kelola memakai cooldown lebih panjang dan kembali ke `Eligible` (bukan langsung `ActiveSet`); validator masih `Probation` tidak dapat ditangguhkan.

### [V4] Integritas Endorsement Anti-Sybil
* Hanya anggota `ActiveSet` dengan masa kerja minimum boleh mengesahkan; target wajib `Probation`/`Eligible`; pengesahan diri dilarang; kuota `max_active_endorsements` dan duplikat tertolak atomik.
* Pasangan timbal balik saling membatalkan bobot (`anti_sybil_weight` -> 0); taint mencabut seluruh endorsement pengesah dan melarang penerbitan ulang.
* Diskon jendela probation per bobot (2.500 BPS per bobot) dengan lantai `1/PROBATION_FLOOR_DIVISOR` dari dasar; slash `>= SEVERE_SLASH_RATE_BPS` menangguhkan pelaku dan menodai pengesahnya.

### [V5] Aritmetika BPS Nir-Pecahan
* `uptime_bps`/`effective_stake_quanta`/`slash_amount` memakai pembagian bilangan bulat ke bawah; akuntansi tak konsisten -> `InvalidBlockAccounting`; luapan -> `ArithmeticOverflow`; tarif > 10.000 BPS -> `InvalidSlashRate`.
* `quorum_weight` mengakumulasi stake efektif `stake * performance_bps / 10.000` dengan `checked_add`; `meets_bps_quorum` membandingkan perkalian murni dan bersifat gagal-tertutup.
* Pemindaian statis `crates/aurion-validator/src/` bebas tipe `f32`/`f64` dan literal pecahan.

---

## 3. ARSITEKTUR TEST SUITE & LOKASI FILE

* **Target Modul:** `crates/aurion-validator/`
* **Lokasi Suite Uji:** `crates/aurion-validator/tests/validator_invariants.rs`
* **Dependensi Pengujian:** `aurion-account` (peran/stake/`RolePromotion`/`SovereignAccount`), `aurion-criptografi` (Ed25519 untuk PoP & otorisasi `unjail`).
* **Permukaan produksi yang dikunci:** `status.rs` (matriks 6 status), `proof.rs` (PoP BLAKE3+Ed25519), `scoring.rs` (BPS `checked_*`), `liveness.rs` (`BlockReport`/`UnjailRequest`), `epoch.rs` (`select_active_set`), `endorsement.rs` (`EndorsementLedger`), `lifecycle.rs` (`ValidatorLifecycle` orkestrator), `error.rs`, `record.rs`.

---

## 4. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

1. Spesifikasi di `docs/task-register/TASK-aurion-validator.md` + terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Matriks V0–V5 di `crates/aurion-validator/tests/validator_invariants.rs` lulus: `cargo test -p aurion-validator --test validator_invariants`.
3. Unit test inline lulus: `cargo test -p aurion-validator`.
4. Audit statis: `cargo clippy -p aurion-validator --all-targets --all-features -- -D warnings` + `python3 tools/aurion_guard.py check`.
5. Tanpa `unsafe`, `f32`/`f64`, `.unwrap()` di `crates/aurion-validator/src/`.
6. Status `COMPLETED` di `logs/WORK_LOG.md`.

---

## 5. HASIL VERIFIKASI (2026-09-29)

| Pemeriksaan | Perintah | Hasil |
| --- | --- | --- |
| Suite invarian V0–V5 | `cargo test -p aurion-validator --test validator_invariants` | 19/19 tes lulus |
| Unit test inline | `cargo test -p aurion-validator` | 1 unit + 19 integrasi lulus |
| Seluruh workspace | `cargo test --workspace` | lulus penuh |
| Clippy ketat modul | `cargo clippy -p aurion-validator --all-targets --all-features -- -D warnings` | bersih |
| Audit statis guard | `python3 tools/aurion_guard.py check` | Zero-Unsafe, Zero-Float, Anti-Unwrap lolos |
| Format & diff | `cargo fmt --all --check`, `git diff --check` | bersih |

### Pemetaan tes ke invarian

| Invarian | Tes | Berkas/Simbol yang dikunci |
| --- | --- | --- |
| V0 | `v0_admission_enforces_candidate_role_and_minimum_stake`, `v0_admission_rejects_duplicate_keys_and_forged_pops_atomically`, `v0_default_policy_matches_protocol_constants_and_rejects_zeroes` | `ValidatorLifecycle::{register, state_digest}`, `proof::ProofOfPossession`, `ValidatorPolicy::validate` |
| V1 | `v1_probation_never_counts_until_k_signed_blocks_are_reported`, `v1_probation_liveness_violation_resets_window_and_cancels_repeated_faults`, `v1_transition_matrix_is_exhaustive_and_probation_cannot_jump` | `status::{ValidatorStatus, can_transition_to, authorize_transition}`, `ValidatorLifecycle::{report_block, graduate}` |
| V2 | `v2_epoch_rotation_only_executes_on_boundary_heights`, `v2_selection_is_deterministic_ordered_and_stable_across_epochs`, `v2_tie_break_is_account_id_and_demotion_returns_eligible` | `epoch::{EpochSchedule, select_active_set}`, `ValidatorLifecycle::{rotate_epoch, selection_preview}` |
| V3 | `v3_active_validator_auto_jails_at_miss_threshold_and_leaves_quorum`, `v3_unjail_requires_cooldown_authorization_and_fresh_nonce`, `v3_suspension_obeys_matrix_and_uses_longer_cooldown` | `liveness::{BlockReport, UnjailRequest}`, `ValidatorLifecycle::{report_block, request_unjail, suspend}` |
| V4 | `v4_endorsement_enforces_tenure_target_and_quota_gates`, `v4_ledger_enforces_quota_duplicate_taint_and_reciprocal_weight`, `v4_endorsements_discount_probation_window_down_to_floor`, `v4_severe_slashing_taints_endorser_and_revokes_weights` | `endorsement::EndorsementLedger`, `ValidatorLifecycle::{endorse, slash, probation_requirement}` |
| V5 | `v5_bps_arithmetic_is_exact_floor_and_fail_closed`, `v5_quorum_weight_and_slash_follow_exact_bps_accounting`, `v5_source_tree_contains_no_floating_point_arithmetic` | `scoring::{uptime_bps, effective_stake_quanta, slash_amount, accumulate_weight, meets_bps_quorum}`, `ValidatorLifecycle::quorum_weight` |

### Catatan perbaikan produksi yang dikunci oleh suite

* `ValidatorStatus::is_selectable` kini mencakup `Eligible | ActiveSet` agar incumbent tetap masuk peringkat seleksi — tanpa ini, rotasi beruntun mengosongkan himpunan aktif pada epoch genap (osilasi) dan suite V2 gagal.
* Seluruh sisanya memakai perilaku yang sudah ada: penolakan atomik, `checked_*`, digest BLAKE3 kanonikal, dan pemindaian sumber statis bebas pecahan.