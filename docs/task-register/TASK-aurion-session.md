# TASK-aurion-session: Browser SessionWallet & Micropayment Channel Tickets (@aurion/sdk)

## 1. Metadata
* **Status:** IN_PROGRESS
* **Package:** `packages/aurion-sdk` (`src/session/`, `src/channel/`, `src/paywall/`)
* **Lingkungan Target:** Browser Modern (Chrome/Firefox/Safari/Edge via WebCrypto & IndexedDB), Node.js (>=18 via `node:crypto`), Edge Runtimes
* **Dependensi:** `@noble/ed25519`, `@noble/hashes`, `@scure/bip39` (tanpa dependensi native biner/C++)
* **Batasan Protokol:** Zero-float murni (`Quanta = bigint`), kanonikal Little-Endian (LE), keselarasan penuh dengan `crates/aurion-channel` (Commit `7697ac1`).

---

## 2. Arsitektur & Prinsip Desain

### 2.1. Zero-Friction Browser SessionWallet
Pengguna tanpa kartu kredit dan tanpa dompet Web3 eksternal (ekstensi) dapat langsung bertransaksi mikro secara instan:
1. **Pembangkitan Otomatis (On-the-Fly):** SDK secara transparan membangkitkan pasangan kunci Ed25519 saat aplikasi dibuka.
2. **Penyimpanan Terisolasi (IndexedDB + WebCrypto):**
   * Private key disimpan terenkripsi di `IndexedDB` lokal menggunakan algoritma AES-GCM 256-bit.
   * Kunci enkripsi AES-GCM diisolasi di memory session atau didelegasikan ke kunci lokal peramban (`non-extractable CryptoKey`).
3. **Penyisihan Memori (Ephemerality):** Kunci sesi dapat berumur pendek (*ephemeral session*) atau persisten per domain/aplikasi mitra.

### 2.2. Format Kanonikal Wire Tiket BalanceProof (128 Byte)
Sesuai verifikasi `crates/aurion-channel` (Commit `7697ac1`), format biner tiket streaming off-chain adalah tepat 128 byte:

```text
 0               8              16                              32                              64                             128 (Bytes)
┌───────────────┬───────────────┬───────────────────────────────┬───────────────────────────────┬───────────────────────────────┐
│  channel_id   │     nonce     │      transferred_amount       │         sender_pubkey         │           signature           │
│   (u64 LE)    │   (u64 LE)    │          (u128 LE)            │          ([u8; 32])           │          ([u8; 64])           │
└───────────────┴───────────────┴───────────────────────────────┴───────────────────────────────┴───────────────────────────────┘
```

* **Pre-image Ditandatangani (Offset 0..64, 64 Byte):**
  * `0..8`: `channel_id` (8 byte, Little-Endian `u64`)
  * `8..16`: `nonce` (8 byte, Little-Endian `u64`)
  * `16..32`: `transferred_amount` (16 byte, Little-Endian `u128`)
  * `32..64`: `sender_pubkey` (32 byte, Ed25519 raw public key)

* **Signature (Offset 64..128, 64 Byte):**
  * Tanda tangan Ed25519 atas 64-byte pre-image di atas.

### 2.3. Client-Side Channel State Manager

Mengelola state saluran lokal sisi klien:

* Memantau `total_deposit`, `current_nonce`, dan `cumulative_transferred`.
* Menolak menandatangani tiket jika `cumulative_transferred + tick_amount > total_deposit` (proteksi overspend lokal).
* Menyediakan API `createTick(deltaAmount)` yang secara otomatis menaikkan `nonce += 1` dan `cumulative_transferred += deltaAmount`.

### 2.4. Pay-Per-Request HTTP Shim (`aurionFetch`)

Abstraksi pemanggil API/konten:

* Menangkap status `HTTP 402 Payment Required` dengan header `WWW-Authenticate: Aurion-Channel id=<channel_id>, amount=<price>`.
* Secara otomatis membangkitkan `BalanceProof` 128-byte baru dan mengirim ulang request dengan header `X-Aurion-Ticket: <base64_encoded_128b_ticket>`.

---

## 3. Matriks Invarian Pengujian (SES0 – SES5)

| ID | Invarian | Batasan & Verifikasi |
| --- | --- | --- |
| **SES0** | **Zero-Float & Monotonicity** | Perhitungan tiket hanya menggunakan `Quanta` (`bigint`). Setiap tiket baru wajib memiliki `nonce > previous_nonce` dan `transferred_amount >= previous_amount`. |
| **SES1** | **Exact 128-Byte Codec** | Serializer `encodeBalanceProof` menghasilkan buffer tepat 128 byte. Deserializer `decodeBalanceProof` memulihkan field integer LE dan public key byte-for-byte. |
| **SES2** | **Rust Channel Conformance** | Pre-image (64B) dan signature (64B) yang dihasilkan SDK diverifikasi lolos validasi `BalanceProof::verify` pada `aurion-channel`. |
| **SES3** | **Deposit Boundary Safety** | `ChannelSession` menolak membuat tiket baru jika nilai kumulatif melebihi `total_deposit`. Melempar `ErrDepositExceeded`. |
| **SES4** | **Keystore Storage Isolation** | `SessionWallet` di browser berhasil menyimpan dan memulihkan keypair dari IndexedDB secara terenkripsi. Private key tidak pernah bocor ke global scope/console logging. |
| **SES5** | **Paywall Interceptor Lifecycle** | Handler `aurionFetch` menangani alur HTTP 402, menyematkan header tiket 128-byte yang valid, dan mengembalikan respons sukses saat server menerima tiket. |

---

## 4. CATATAN IMPLEMENTASI

1. **Keselarasan wire format:** Implementasi SDK mengikuti persis layout 128-byte yang diverifikasi di `crates/aurion-channel` (commit `7697ac1`). Pre-image 64-byte = `channel_id(8) + nonce(8) + transferred(16) + sender_pubkey(32)`, signature Ed25519 64-byte.
2. **Zero-float:** Seluruh perhitungan moneter menggunakan `bigint` (Quanta). Tidak ada `number` di jalur uang.
3. **Storage isolation:** Private key dienkripsi AES-GCM 256-bit sebelum disimpan ke IndexedDB. Kunci enkripsi tidak pernah disimpan bersama ciphertext.
4. **Paywall flow:** `aurionFetch` menangkap HTTP 402, parse `WWW-Authenticate` header, generate tiket 128-byte, dan retry request dengan header `X-Aurion-Ticket`.

---

## 5. Verifikasi
- `npm --prefix packages/aurion-sdk run typecheck`
- `npm --prefix packages/aurion-sdk run test`
- `npm --prefix packages/aurion-sdk run build`
- `python3 tools/aurion_guard.py check` (exit 0)