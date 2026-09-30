# TASK-consensus-master: Production BFT Consensus Pipeline with Mempool Harvesting & Concrete Block Hash Voting

## 1. Metadata
* **Status:** IN_PROGRESS
* **Scope:** `apps/aurion-node` (`src/consensus/`, `src/mempool/`, `src/engine/`), `crates/aurion-consensus`, `crates/aurion-ledger`, `crates/aurion-execution`
* **Dependensi Internal:** `aurion-core`, `aurion-criptografi`, `aurion-ledger`, `aurion-execution`, `aurion-mempool`
* **Batasan Protokol Mutlak:**
  * `#![forbid(unsafe_code)]` di seluruh modul baru.
  * Zero-Float murni: dilarang keras tipe `f32`/`f64`. Moneter murni `Quanta = u128` ($10^{10} = 1\text{ AUR}$).
  * Kanonikal Little-Endian (LE) untuk seluruh serialisasi integer wire.
  * Kuorum BFT 3-of-4 ($N = 4, f = 1 \implies 2f + 1 = 3$).
  * Suara konsensus mengikat hash blok konkret (`block_hash`), bukan penanda height abstrak/sintetis.

---

## 2. Arsitektur Pipeline Konsensus-Master

Pipeline konsensus mengakhiri fase *synthetic voting* dan beralih ke validasi blok produksi melalui 4 tahap atomik:

```text
  [MEMPOOL] ──► Pemanenan Tx (Maks N tx)
                     │
                     ▼
  [LEADER]  ──► Eksekusi Spekulatif (TransactionalCache)
                Hitung post_state_root + tx_root
                Bentuk BlockHeader (116B) & BlockProposal Wire
                Siarkan via Zenoh: aurion/consensus/proposal
                     │
                     ▼
[VALIDATOR] ──► Eksekusi Ulang Independen (TransactionalCache)
                Verifikasi: calculated_state_root == proposal.header.state_root
                Jika Valid: Terbitkan PrecommitVote mengikat block_hash konkret
                Siarkan via Zenoh: aurion/consensus/vote
                     │
                     ▼
[COMMITTER] ──► Kumpulkan Kuorum 3-of-4 Quorum Certificate (QC)
                Atomic Commit ke redb:
                  - BLOCKS_TABLE: Tulis BlockHeader + Tx List
                  - ACCOUNTS_TABLE: Tulis saldo mutakhir [u8; 24]
                Kuras transaksi terkonfirmasi dari Mempool
```

---

## 3. Spesifikasi Biner Wire Format

### 3.1. Kanonikal 116-Byte BlockHeader

Header blok Aurion terdiri atas 6 field kanonikal berukuran tetap:

| Offset (Byte) | Field | Tipe | Deskripsi |
| --- | --- | --- | --- |
| `0..4` | `chain_id` | `u32` (LE) | Pengenal jaringan unik (misal: 1001) |
| `4..12` | `height` | `u64` (LE) | Tinggi blok monolitik terurut |
| `12..20` | `timestamp` | `u64` (LE) | Waktu blok (Unix Epoch detik) |
| `20..52` | `parent_hash` | `[u8; 32]` | BLAKE3 hash dari BlockHeader sebelumnya |
| `52..84` | `state_root` | `[u8; 32]` | Root hash keadaan akun pasca-eksekusi batch |
| `84..116` | `tx_root` | `[u8; 32]` | Root hash transaksi (BLAKE3 berantai / merkle) |

**Total Panjang Header: Tepat 116 Byte.**

### 3.2. Wire Format `BlockProposal` (Daftar Transaksi Nyata)

Pesan proposal blok disiarkan oleh Leader ke seluruh validator:

```text
 0               4              120             124              124 + (N × 168)     188 + (N × 168)
┌───────────────┬───────────────┬───────────────┬───────────────────────────────┬───────────────────┐
│ magic_prefix  │  block_header │   tx_count    │      tx_envelopes (168B)      │ proposer_signature│
│   (4B LE)     │    (116B)     │   (u32 LE)    │           (N × 168B)          │      (64B)        │
│  0x41555250   │               │               │                               │                   │
└───────────────┴───────────────┴───────────────┴───────────────────────────────┴───────────────────┘
```

1. **Magic Prefix (4 Byte):** `0x41555250` (ASCII: `"AURP"`).
2. **Block Header (116 Byte):** Struktur biner kanonikal 116B.
3. **Transaction Count (4 Byte):** `u32` LE menentukan jumlah transaksi $N$.
4. **Transaction Payload ($N \times 168$ Byte):** Sekuens biner transaksi 168-byte tanpa pemisah.
5. **Proposer Signature (64 Byte):** Tanda tangan Ed25519 leader atas pre-image: `magic_prefix (4B) || block_header (116B) || tx_count (4B) || tx_root (32B)`.

### 3.3. Wire Format `PrecommitVote` (140 Byte)

Pesan suara validator yang mengikat hash blok konkret:

```text
 0               4              12              44                              76                             140
┌───────────────┬───────────────┬───────────────┬───────────────────────────────┬───────────────────────────────┐
│     round     │    height     │  block_hash   │         voter_pubkey          │           signature           │
│   (u32 LE)    │   (u64 LE)    │  ([u8; 32])   │          ([u8; 32])           │          ([u8; 64])           │
└───────────────┴───────────────┴───────────────┴───────────────────────────────┴───────────────────────────────┘
```

* **Pre-image Suara (76 Byte):** `round (4B LE) || height (8B LE) || block_hash (32B) || voter_pubkey (32B)`.
* **Signature (64 Byte):** Ed25519 signature validator atas pre-image 76 byte di atas.

### 3.4. Struktur `QuorumCertificate` (QC)

QC dibentuk setelah terkumpul $\ge 3$ suara valid dari set validator aktif ($N=4$):

* `block_hash`: `[u8; 32]`
* `height`: `u64`
* `round`: `u32`
* `signatures`: `Vec<([u8; 32], [u8; 64])>` (Daftar public key validator dan tanda tangan, minimal 3 pasang).

---

## 4. Matriks Invarian Pengujian (CM0 – CM5)

| ID | Invarian | Batasan & Verifikasi |
| --- | --- | --- |
| **CM0** | **Wire Codec Roundtrip & Zero-Tx Packing** | Proposal tanpa transaksi ($N=0$) berukuran tepat 184 byte ($4 + 116 + 4 + 0 + 64$). Serialisasi dan deserialisasi proposal lolos uji kesetaraan byte-for-byte. |
| **CM1** | **Speculative StateRoot Consensus Parity** | `state_root` yang dihitung oleh leader wajib identik dengan hasil eksekusi independen validator. Jika ada selisih 1 bit, proposal ditolak. |
| **CM2** | **Strict Concrete Hash Binding** | Suara validator menolak proposal dan tidak akan diterbitkan jika `block_hash` tidak diturunkan dari BLAKE3 header yang valid. Suara abstrak berbasis height ditolak. |
| **CM3** | **Byzantine Fault Tolerance (3-of-4)** | Blok sah dieksekusi jika dan hanya jika terdapat minimal 3 suara valid dari 4 validator terdaftar. Sistem tetap berjalan jika 1 node mati/byzantine, dan mogok aman (*safety halt*) jika $\ge 2$ node bermasalah. |
| **CM4** | **Atomic Ledger Commit & State Persistence** | Penulisan blok ke `BLOCKS_TABLE` dan pembaruan saldo ke `ACCOUNTS_TABLE` dieksekusi dalam satu transaksi atomik `redb`. Kegagalan parsial membatalkan seluruh blok. |
| **CM5** | **Fail-Fast Invalid Tx Batch & Mempool Purge** | Jika salah satu transaksi dalam batch memiliki tanda tangan cacat, nonce basi, atau saldo kurang, proposal ditolak utuh. Transaksi blok yang sukses langsung dihapus dari mempool. |

---

## 5. CATATAN IMPLEMENTASI

1. **Header format baru:** `ConsensusBlockHeader` 116-byte dengan `chain_id`, `height`, `timestamp`, `parent_hash`, `state_root`, `tx_root`. Format ini berbeda dari `BlockHeader` di `aurion-core` dan khusus untuk lapisan konsensus.
2. **Magic prefix:** `0x41555250` (ASCII `"AURP"`) sebagai penanda wire format proposal.
3. **PrecommitVote:** 140-byte fixed-size dengan pre-image 76-byte yang mengikat `block_hash` konkret.
4. **QuorumCertificate:** Minimal 3 suara valid dari 4 validator untuk BFT 3-of-4.
5. **Atomic commit:** Penulisan blok dan update saldo dalam satu transaksi `redb`.

---

## 6. Verifikasi
- `cargo check --workspace --all-targets`
- `cargo test -p aurion-consensus -p aurion-node`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all --check`
- `python3 tools/aurion_guard.py check` (exit 0)