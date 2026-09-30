# TASK: task-aurion-genesis
> **Target Modul:** `aurion-genesis` (`crates/aurion-genesis`) + `aurion-execution` (`crates/aurion-execution`)
> **Kategori:** Bootstrap State Root & Mesin Replenishment Pasokan Siklus
> **Status:** COMPLETED (2026-09-30)
> **Otoritas:** `Agents.md`
> **Tingkat Ketergantungan:** 1 (`aurion-core`, `aurion-validator`, `aurion-execution`, `aurion-ledger`)

---

## 1. Tujuan pengujian

Memvalidasi bootstrap genesis Model A (Blok Sintetis #0) deterministik dan mesin
`ReplenishmentEngine` (pencetakan pasokan siklus saat Treasury kosong) dengan
invariant zero-float, `Quanta = u128`, dan konservasi pasokan yang ketat.

## 2. Matriks invarian GEN0–GEN5 dan hasil

- **GEN0 — Keketatan skema:** parser skema JSON menolak field tak dikenal
  (`deny_unknown_fields`), menolak representasi float, dan menolak angka
  negatif; hanya string desimal atau integer tak-bertanda yang diterima untuk
  seluruh nilai `u128`.
- **GEN1 — State root deterministik:** dua `GenesisConfig` berisi data identik
  dengan urutan alokasi diacak menghasilkan `state_root` identik byte-per-byte
  (pengurutan leksikografis biner murni sebelum hashing).
- **GEN2 — Blok Sintetis #0:** header `height = 0`, `prev_hash = [0; 32]`,
  `tx_count = 0`, proposer nol mutlak, tanpa transaksi; `state_root` pada
  header cocok byte-per-byte dengan kalkulasi.
- **GEN3 — Konservasi moneter & split 21%:** total alokasi + stake validator
  konservatif terhadap `660.000.000.000.000.000` Quanta; akun Creator menerima
  tepat `138.600.000.000.000.000` Quanta (21%) dan Reservoir `79%`; penyimpangan
  ditolak dengan `SupplyConservationMismatch`.
- **GEN4 — Himpunan validator perdana:** genesis dengan 0 validator ditolak
  (`ZeroValidators`); duplikasi `consensus_pubkey` ditolak
  (`DuplicateValidatorPubkey`).
- **GEN5 — Trigger replenishment:** saldo Treasury nol memicu kredit tepat
  66.000.000 AUR (`TREASURY_GENESIS_QUANTA`) dan `cycle_index` bergerak `0 → 1`;
  saldo positif tidak memicu pencetakan.

## 3. Lokasi dan dependensi suite

- **Suite GEN0–GEN5:** `crates/aurion-genesis/tests/genesis_invariants.rs`
- **Suite regresi replenishment:** `crates/aurion-execution/tests/execution_invariants.rs`
- **Implementasi baru:**
  - `crates/aurion-genesis/src/config.rs` — skema `GenesisConfig`,
    `ConsensusGenesisParams`, `GenesisAllocation`, `GenesisValidator`, helper
    `u128`-sebagai-string, validasi konservasi pasokan & himpunan validator.
  - `crates/aurion-genesis/src/builder.rs` — `compute_genesis_state_root`
    (derive-key BLAKE3 `AURION_GENESIS_STATE_ROOT_V1`), `build_synthetic_block_zero`,
    `verify_genesis_state_root`.
  - `crates/aurion-genesis/src/error.rs` — varian `SchemaViolation`,
    `SupplyConservationMismatch`, `StateRootMismatch`, `ZeroValidators`,
    `DuplicateValidatorPubkey`, `SerializationFailure`.
  - `crates/aurion-execution/src/replenishment.rs` — `ReplenishmentEngine`
    (dipublikasikan ulang dari `aurion-execution::lib`).

## 4. Perubahan runtime yang diperlukan

Tidak ada. `ReplenishmentEngine` sengaja tidak di-hot-wire ke jalur eksekusi
blok hidup node (keputusan cakupan: modular + pengujian); node tetap memakai
`GenesisBootstrap`/`GenesisSpec` eksisting. Konsumsi pasokan siklus dapat
diintegrasikan di masa depan tanpa mengubah signature engine.

## 5. Konstanta kanonikal

- `QUANTA_PER_AUR = 10_000_000_000` (re-export dari `aurion-core::types`)
- `CYCLE_SUPPLY_AUR = 66_000_000`
- `TOTAL_CYCLE_SUPPLY_QUANTA = 660_000_000_000_000_000`
- `CREATOR_ALLOCATION_BPS = 2_100` (21%), `RESERVOIR_ALLOCATION_BPS = 7_900` (79%)
- `CREATOR_ALLOCATION_QUANTA = 138_600_000_000_000_000`,
  `RESERVOIR_ALLOCATION_QUANTA = 521_400_000_000_000_000`

## 6. Referensi silang

- Skema kunci state root: `blake3("account"/"validator"/"system")` +
  awalan label; lih. `builder.rs`.
- Kebijakan zero-float & anti-unwrap: `Agents.md` §1.2.
- Persistensi: `TASK-aurion-ledger.md` (L3 append-only, genesis tinggi 0).