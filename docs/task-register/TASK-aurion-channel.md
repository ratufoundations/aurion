# TASK-aurion-channel: Native Micropayment & State Channel Engine (Pengganti aurion-contract)

## 1. Metadata
* **Status:** IN_PROGRESS
* **Crate:** `crates/aurion-channel`
* **Dependensi:** `aurion-core`, `aurion-criptografi`, `aurion-execution`
* **Batasan Protokol:** `#![forbid(unsafe_code)]`, zero-float murni (dilarang keras `f32`/`f64`), moneter murni `Quanta = u128` ($10^{10} = 1\text{ AUR}$).
* **Pola Arsitektur:** RFC-001 Native Keeper (Capability-based state isolation).

---

## 2. Arsitektur & Mekanisme Saluran
1. **Lifecycle Saluran (3 Fase Atomik):**
   * **Open (On-Chain):** Inisiator mengunci saldo escrow via transaksi 168-byte ke alamat keeper saluran. Saldo masuk ke tabel terisolasi `channels`.
   * **Streaming Ticks (Off-Chain via Zenoh/P2P):** Klien mengirimkan tiket `BalanceProof` bertanda tangan Ed25519 langsung ke penerima tanpa melibatkan konsensus atau biaya gas per transaksi.
   * **Close / Settle (On-Chain):** Salah satu pihak mengajukan tiket `BalanceProof` dengan nonce tertinggi untuk mendistribusikan sisa escrow secara atomik. Penyelesaian bersama (*cooperative*, kedua pihak menandatangani) langsung tuntas; penyelesaian sepihak membuka jendela sanggah.
2. **Proteksi Timeout & Dispute Window:**
   * Jika penutupan dilakukan sepihak (*unilateral close*), saluran memasuki masa sanggah (*challenge window*) selama $N$ blok (misal: 100 blok) sebelum dana dapat dicairkan, guna mencegah kecurangan pengajuan tiket lama (*replay of stale proofs*). Gugatan dengan nonce lebih tinggi menyanggah tiket usang dan memperbarui state penyelesaian.

---

## 3. Format Wire Kanonikal Off-Chain `BalanceProof` (128 Byte)

Lebar tiket streaming off-chain: `channel_id(8) + nonce(8) + transferred(16) + sender_pubkey(32) + signature(64) = **128 byte**` dengan integer **Little-Endian**.

> **Catatan koreksi draft:** Header draft menyebut "120 Byte" dan preimage "56 Byte". Aritmetika tersebut keliru: `8+8+16+32+64 = 128`, dan preimage `8+8+16+32 = 64`. Implementasi memakai angka kanonikal 128-byte / preimage 64-byte.

```text
 0               8              16              32                              64                              128 (Bytes)
┌───────────────┬───────────────┬───────────────┬───────────────────────────────┬───────────────────────────────┐
│  channel_id   │     nonce     │  transferred  │        sender_pubkey          │           signature           │
│   ([u8; 8])   │   (u64 LE)    │  (u128 LE)    │          ([u8; 32])           │          ([u8; 64])           │
└───────────────┴───────────────┴───────────────┴───────────────────────────────┴───────────────────────────────┘
```

* **Payload yang Ditandatangani (Pre-image, 64 Byte):** `channel_id (8B) + nonce (8B LE) + transferred (16B LE) + sender_pubkey (32B)`.
* **Tanda Tangan:** 64 byte Ed25519 menggunakan kunci privat sender (diverifikasi via `aurion-criptografi::verifikasi_tanda_tangan`).

---

## 4. Matriks Invarian Pengujian (CH0 – CH5)

| ID | Invarian | Batasan & Verifikasi |
|---|---|---|
| **CH0** | **Zero-Float & Monotonicity** | Nilai kumulatif transfer bertipe `u128`. Setiap `BalanceProof` baru wajib memiliki `nonce > previous_nonce` dan `transferred_amount >= previous_amount`. |
| **CH1** | **Atomic Escrow Lock** | Pembukaan saluran mendebit saldo partisipan dan mengkredit ke escrow saluran secara atomik di `TransactionalCache`. Kegagalan parsial memicu rollback total. |
| **CH2** | **Strict Value Conservation** | Pada saat penutupan saluran: `total_payout_receiver + refund_sender == initial_deposit`. Tidak boleh ada Quanta yang tercipta atau lenyap. |
| **CH3** | **Ed25519 Ticket Verifiability** | `BalanceProof` wajib menolak payload jika tanda tangan 64-byte tidak cocok dengan public key inisiator atas digest biner tiket. |
| **CH4** | **Unilateral Challenge Resolution** | Jika tiket tandingan yang lebih baru diajukan dalam jendela sanggah (`nonce_baru > nonce_lama`), status penyelesaian langsung diperbarui ke state paling mutakhir. |
| **CH5** | **Zero-State Leakage (RFC-001)** | Keeper saluran hanya berhak memutasi namespace kuncinya sendiri (`StoreKey::new("channel")`). Dilarang menyentuh saldo di luar hak kapabilitas keeper. |

---

## 5. CATATAN IMPLEMENTASI (DEVIASI & KEPUTUSAN)

1. **Koreksi aritmetika wire-format (PENTING).** Draft "120 Byte" / preimage "56 Byte" keliru; kanonikal = **128 byte** tiket dengan preimage **64 byte** (lihat §3). Jenjang offset di tabel draft (0/8/24/32/64/128) juga digeser menjadi 0/8/16/32/64/128 agar sel `nonce (u64 LE)` dan `transferred (u128 LE)` berimbuh: nonce `8..16`, transferred `16..32`.
2. **Namespace kanonikal via `StoreKey`.** CH5 menuliskan `blake3("channel") || b"chan:" || channel_id`. Eksekusi memakai digest berdomain `StoreKey::new("channel")` (BLAKE3 domained `AURION_STORE_KEY_NAMESPACE_V1`), sejalan dengan namespace `account`/`staking`. Kunci record = `namespace_digest || b"chan:" || channel_id`. Invarian CH5 (tidak boleh menulis kunci asing) dipertahankan; bentuk digest menyusul konvensi eksekusi.
3. **Escrow = address semu berdomain di namespace akun; metadata di namespace channel.** Debit peserta dilakukan via `AccountKeeper::transfer(sender → escrow_account)` dengan `escrow_account = blake3("AURION_CHANNEL_ESCROW_V1" || channel_id)[..32]`. Escrow tercatat sebagai saldo akun semu (account namespace), sedangkan lifecycle/channel state berada di namespace `channel` (`chan:`). Ini mengikuti pola authorized cross-keeper (bandingkan `StakingKeeper::lock_stake`): mutasi foreign key hanya lewat metode pemilik namespace, tidak pernah `write_raw`/`cache.set` langsung (CH5).
4. **`submit_close` bersifat sepihak; penyelesaian bersama memerlukan signature receiver.** Satu tiket 128-byte hanya membawa tanda tangan sender, sehingga "kesepakatan bersama" tidak dapat dibuktikan dari tiket tunggal. `submit_close(cache, proof, current_height)` selalu membuka jendela sanggah (semantik unilateral §2.2); `close_cooperatively(cache, proof, receiver_signature, current_height)` memverifikasi kedua tanda tangan atas preimage 64-byte yang sama lalu langsung mendistribusikan dana dan menandai `Settled`.
5. **`channel_id` deterministik.** `channel_id = blake3("AURION_CHANNEL_ID_V1" || sender || receiver || deposit_le || challenge_blocks_le)[..8]`. Dua open identik menghasilkan id sama → `ChannelAlreadyExists` (idempotensi). Beda deposit/blocks → id berbeda.
6. **Serialisasi state 130-byte.** Record channel dalam namespace: versi (u8) + status (u8) + channel_id (8) + sender (32) + receiver (32) + total_deposit (16 LE) + settled_amount (16 LE) + last_nonce (8 LE) + challenge_period (8 LE) + expire_height (8 LE, `0` bila Open/Settled). Determinis dan leksikografis stabil.
7. **Varian error tambahan.** `InsufficientSenderBalance { available, required }` (CH1: saldo kurang saat open), `ChannelSettled`, `NotInChallengeWindow` — di luar delapan varian dasar draft namun diperlukan untuk invariants.
8. **`crates/aurion-contract` DIHAPUS.** VM eksperimental telah dihapus dari workspace (commit pembersihan); mesin micro-payment native `aurion-channel` menggantikan arah arsitektur VM Wasm secara permanen.

---

## 6. Verifikasi
- `cargo check -p aurion-channel`, `cargo clippy -p aurion-channel --all-targets --all-features -- -D warnings`, `cargo test -p aurion-channel` (CH0–CH5).
- `cargo fmt --all --check`, `python3 tools/aurion_guard.py check` (exit 0).
- `cargo test --workspace` / `cargo clippy --workspace` — catat regresi pre-existing bila ada.