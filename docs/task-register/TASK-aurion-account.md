# TASK: task-aurion-account
> **Target Modul:** `aurion-account` (`crates/aurion-account`)
> **Kategori:** Modul Identitas Berdaulat, Kebijakan Akun & Kontrol Akses (Identity & Policy Engine)
> **Status:** COMPLETED (2026-09-29) — suite A0–A5 hijau, audit statis lolos
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper)
> **Tingkat Ketergantungan:** 2 (`aurion-core`, `aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-account` mengatur kedaulatan identitas (*SovereignAccount*), pemisahan peran (*roles*), otorisasi berbasis kapabilitas (*Capability-Based Keeper* sesuai RFC-001), dan aturan mutasi kebijakan (rotasi kunci serta multi-signature). Modul ini menetapkan batasan hak istimewa antara pengguna biasa (*Standard User*), validator konsensus (*Validator*), dan dewan pengawas (*GuardCouncil*).

Kegagalan pada modul ini dapat berakibat fatal: eskalasi hak akses (*privilege escalation*), pengambilalihan akun secara ilegal (*account takeover*), serangan *replay* pada delegasi tanda tangan, atau pembengkakan memori penyimpanan akibat akun sampah (*state bloat*).

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-account` secara terisolasi** agar:
1. Memastikan derivasi alamat akun dari public key Ed25519 bersifat deterministik, satu arah, dan dilindungi oleh pemisahan domain kriptografis (*domain separation tag*).
2. Menegakkan isolasi peran secara mutlak: akun tanpa peran/kapabilitas yang sah ditolak ketika mencoba memicu aksi istimewa (*privileged actions*).
3. Menjamin mutasi kebijakan akun (rotasi kunci penandatangan, pengubahan ambang batas multi-sig) berlangsung secara atomik dan kebal terhadap celah penguncian diri (*bricking*) maupun pembajakan (*takeover*).
4. Mencegah eksploitasi tanda tangan delegasi dan multi-signature lintas rantai, lintas akun, atau lintas transaksi (*replay protection*).
5. Mencegah serangan *state bloat* melalui pengelolaan siklus hidup akun nir-saldo (*zero-balance/dust*) yang terprediksi.
6. Mengunci seluruh perhitungan kuota, batas penarikan, ambang batas persetujuan, dan bobot hak suara pada bilangan bulat murni `u128` Quanta (*zero-float*).
7. Menegakkan disiplin kode: `#![forbid(unsafe_code)]`, zero-float, penolakan `.unwrap()` di jalur produksi, serta penanganan error bertipe kuat melalui `thiserror`.

---

## 2. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (A0-A5)

### [A0] Derivasi Alamat Berdaulat & Pemisahan Domain
* `AccountId` 32-byte diturunkan deterministik dari public key Ed25519 via BLAKE3 dengan tag kanonikal `AURION_ADDR_CANONICAL_V1`.
* Tanpa tag / tag salah -> digest berbeda (*domain collision resistance*); 1 bit flip public key -> ~50% bit alamat berubah.
* Parser menolak slice bukan 32 byte tanpa panic.

### [A1] Pemisahan Peran & Penegakan Kapabilitas RBAC
* Peran: `StandardUser`, `ValidatorCandidate`, `ActiveValidator`, `GuardCouncil`.
* `StandardUser` dilarang vote/proposal/slashing -> `Err(AccountError::UnauthorizedRole { expected, actual })`.
* Promosi sah hanya bila staking/deposit minimum `u128` Quanta + otorisasi berhak; demosi seketika membatalkan hak istimewa.

### [A2] Keamanan Mutasi Kebijakan & Rotasi Kunci
* Rotasi kunci wajib bukti otorisasi kunci aktif lama; threshold multi-sig: M >= 1, M <= N, signer N tanpa duplikat.
* Pelanggaran -> `Err(AccountError::InvalidThreshold)`; mutasi yang me-brick akun ditolak di validasi transisi.

### [A3] Proteksi Replay Delegasi & Multi-Sig
* Persetujuan mengikat `chain_id`, `account_address`, `account_nonce`, `action_digest`.
* Chain salah -> `InvalidChainId`; akun salah -> `SignerMismatch`; nonce dipakai ulang -> replay ditolak; eksekusi sukses menaikkan nonce atomik.

### [A4] Pertahanan State Bloat & Akun Nir-Saldo
* Akun baru hanya terdaftar bila transfer awal >= `MIN_ACCOUNT_RESERVE_QUANTA`.
* Di bawah dust ke akun belum ada -> `Err(AccountError::BelowDustThreshold)`.
* Saldo 0 + nonce selesai -> kandidat pruning / representasi kanonikal tetap.

### [A5] Akuntansi Kuota & Batas Nir-Pecahan
* Kuorum multi-sig >= 67% via integer: `(accumulated_weight * 100) >= (total_weight * threshold_percent)`.
* Akumulasi memakai `checked_add`/`checked_sub`; tidak ada `f32`/`f64` di `crates/aurion-account/src/`.

---

## 3. ARSITEKTUR TEST SUITE & LOKASI FILE

* **Target Modul:** `crates/aurion-account/`
* **Lokasi Suite Uji:** `crates/aurion-account/tests/account_invariants.rs`
* **Dependensi Pengujian:** `aurion-core` (tipe dasar), `aurion-criptografi` (Ed25519 + multi-sig).

---

## 4. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

1. Spesifikasi di `docs/task-register/TASK-aurion-account.md` + terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Matriks A0-A5 di `crates/aurion-account/tests/account_invariants.rs` lulus: `cargo test -p aurion-account --test account_invariants`.
3. Unit test inline lulus: `cargo test -p aurion-account`.
4. Audit statis: `cargo clippy -p aurion-account --all-targets --all-features -- -D warnings` + `python3 tools/aurion_guard.py check`.
5. Tanpa `unsafe`, `f32`/`f64`, `.unwrap()` di `crates/aurion-account/src/`.
6. Status `COMPLETED` di `logs/WORK_LOG.md`.

---

## 5. HASIL VERIFIKASI (2026-09-29)

| Pemeriksaan | Perintah | Hasil |
| --- | --- | --- |
| Suite invarian A0–A5 | `cargo test -p aurion-account --test account_invariants` | 16/16 tes lulus |
| Unit test inline | `cargo test -p aurion-account` | 1 unit + 16 integrasi lulus |
| Clippy ketat modul | `cargo clippy -p aurion-account --all-targets --all-features -- -D warnings` | bersih |
| Clippy ketat workspace | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | bersih |
| Audit statis guard | `python3 tools/aurion_guard.py check` | Zero-Unsafe, Zero-Float, Anti-Unwrap lolos |
| Format & diff | `cargo fmt --all --check`, `git diff --check` | bersih |

### Pemetaan tes ke invarian

| Invarian | Tes | Berkas/Simbol yang dikunci |
| --- | --- | --- |
| A0 | `a0_account_id_derivation_is_deterministic_domain_separated_and_avalanching`, `a0_account_id_parser_is_lossless_and_rejects_malformed_lengths` | `address::{derive_account_id, derive_account_id_with_domain, parse_account_id, ADDRESS_DOMAIN_TAG}` |
| A1 | `a1_role_separation_rejects_unauthorized_privileged_actions`, `a1_promotion_requires_stake_signature_and_master_authority`, `a1_demotion_revokes_privileges_immediately` | `role::{Role, RoleAction}`, `SovereignAccount::{promote_to_candidate, activate_validator, demote_to_standard_user}` |
| A2 | `a2_multisig_threshold_construction_rejects_bricked_shapes`, `a2_approval_collection_requires_distinct_signers_meeting_threshold`, `a2_key_rotation_requires_old_key_proof_and_preserves_authority`, `a2_bricked_policy_mutation_is_rejected_without_touching_state` | `multisig::MultiSigPolicy`, `SovereignAccount::{rotate_master_key, update_multisig_policy}` |
| A3 | `a3_delegation_binding_rejects_cross_chain_account_and_action_replay`, `a3_nonce_advances_atomically_and_replay_is_rejected` | `delegation::{DelegatedApproval, DelegationContext, DelegationLedger}` |
| A4 | `a4_new_account_requires_reserve_while_dust_to_existing_is_allowed`, `a4_zero_balance_accounts_are_canonical_and_prunable` | `lifecycle::{AccountLifecycle, MIN_ACCOUNT_RESERVE_QUANTA}`, `SovereignAccount::{register, state_digest}` |
| A5 | `a5_integer_quorum_math_is_exact_and_overflow_safe`, `a5_quota_accounting_uses_checked_integer_arithmetic`, `a5_sources_are_free_of_floating_point_tokens` | `policy::{meets_integer_quorum, SpendingPolicy}`, pemindaian `src/` bebas `f32`/`f64` |

---

## 6. Fakta kanonikal dimensi fisik akun (sinkron 2026-09-30)

- Representasi persisten satu akun adalah **`[u8; 24]`** (`encode_account` di
  `aurion-ledger/src/codec.rs`): 16 byte saldo `u128` LE + 8 byte nonce `u64` LE.
- Pada `aurion-ledger`, tabel akun berbentuk
  `TableDefinition<&[u8; 32], &[u8; 24]>` (`ACCOUNTS_TABLE`) — key `AccountId`
  32 byte, value 24 byte. Karena itu saldo/nonce akun selalu dibaca-tulis dalam
  blok 8/16 byte, konsisten dengan seluruh kodec transaksi 168 byte.
- `MIN_ACCOUNT_RESERVE_QUANTA = 1_000` Quanta (A4) dan
  `MIN_VALIDATOR_STAKE_QUANTA = 1_000_000` Quanta (`aurion-account/src/`),
  keduanya `u128`; akun baru di bawah ambang ditolak (`BelowDustThreshold`).

