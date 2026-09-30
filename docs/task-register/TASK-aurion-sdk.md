# TASK: task-aurion-sdk
> **Target Modul:** `packages/aurion-sdk` (`@aurion/sdk` — client SDK TypeScript)  
> **Kategori:** Client SDK, Wire Codec, Zero-Float Monetary, Transport  
> **Status:** ACTIVE (IN PROGRESS — Fase 1 init: scaffolding, core, codec, kriptografi, transport, client, dan suite unit SDK0–SDK5 selesai)  
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper), kodeks wire kanonikal `crates/aurion-ledger/src/codec.rs`  
> **Tingkat Ketergantungan:** 1 (kodeks wire kanonikal `aurion-ledger`; referensi penamaan jalur & bentuk respon dari `aurion-gateway`, derivasi alamat dari `aurion-account`)

---

## 1. TUJUAN & FILOSOFI

`@aurion/sdk` adalah antarmuka antara ekosistem Rust Aurion dan klien pihak ketiga
(dompet, explorer, bot, aplikasi). Karena seluruh rantai berpegang pada disiplin
**zero-float** (nilai moneter hanya `u128 Quanta`) dan **envelope 168-byte kanonikal**,
SDK mewarisi disiplin yang sama ke sisi TypeScript agar tidak ada jalur yang
melewatkan `IEEE-754 double` untuk uang.

Filosofi SDK:

1. **Nilai moneter selalu `bigint` (Quanta).** Tidak ada `number` di jalur uang;
   parser desimal AUR → Quanta bersifat ketat (menolak eksponen/float, presisi >10 desimal).
2. **Envelope satu-satunya kanal pembayaran.** Klien hanya mengirim satu bentuk byte
   eksak (168 byte), memudahkan verifikasi lintas bahasa menuju verifikator Rust
   (`ed25519-dalek`, `redb` ledger).
3. **Sig lebih dulu di sisi klien (offline).** `signTransfer` tidak memerlukan jaringan;
   transport hanya mem-persist hasil tanda tangan.
4. **Fail-fast.** Envelope dengan panjang salah, fee kurang dari minimum, amount nol,
   atau mnemonic tidak valid langsung menolak di level SDK, bukan menunggu gateway.
5. **Deterministik & tanpa efek samping.** Pure function untuk kodek/tanda tangan;
   `HttpTransport` adalah implementasi default yang tergantikan lewat `AurionTransport`.

---

## 2. ARSITEKTUR PAKET

```text
packages/aurion-sdk/
├── src/
│   ├── index.ts                     # permukaan publik SDK (re-export, versi)
│   ├── errors.ts                    # AurionSdkError + kode galat terstruktur
│   ├── core/
│   │   ├── constants.ts             # QUANTA_PER_AUR=10^10, ENVELOPE_SIZE=168, domain tag, dst.
│   │   └── quanta.ts                # chip moneter Quanta (BigInt, zero-float) [SDK4]
│   ├── codec/
│   │   ├── binary.ts                # read/write u64/u128 LE & BE (konformans)
│   │   └── envelope.ts              # buildSigningPayload(104B) / encode(168B) / decode [SDK1]
│   ├── crypto/
│   │   └── keypair.ts               # Ed25519: generate/fromPrivateKey/fromMnemonic/derive [SDK2]
│   ├── transport/
│   │   ├── transport.interface.ts   # AurionTransport, TxReceipt, AccountResponse [SDK3]
│   │   └── http-shim.ts             # HttpTransport (fetch; route Zenoh-shim) [SDK3]
│   └── client/
│       ├── types.ts                 # TransferParams, BalanceParams, NonceProvider
│       └── aurion-client.ts         # AurionClient: getBalance/getAccount/signTransfer/transfer [SDK1–5]
└── tests/
    ├── quanta.test.ts               # invarian SDK4 (28 test total, 0 gagal)
    ├── envelope.test.ts             # invarian SDK1–SDK2
    └── keypair.test.ts              # invarian SDK2
```

Alur transfer:

```text
   [Client AURION]                              [Chain / Gateway]
        │ signEnvelope (preimage 104B + sig 64B)         │
        │ encodeEnvelope → 168 byte                      │
        │        GET  aurion/{chainId}/account/{pkHex}   │──► { address, balance, nonce }
        │        POST aurion/{chainId}/tx/submit         │──► { txHash: BLAKE3(envelope) }
        ▼                                                ▼
   AurionClient.signTransfer → keypair.verify → encode → HttpTransport (octet-stream)
```

---

## 3. SPESIFIKASI INVARIAN & MATRIKS UJI (SDK0 – SDK5)

### SDK0 — Bootstrap & Kemasan
- Paket `@aurion/sdk` v0.1.0, `type: module`, dual **ESM (`dist/index.js`) + CJS (`dist/index.cjs`)** + `dist/index.d.ts`/`.d.cts` via `tsup`.
- Target `ES2022`, `strict` TS, `noEmit` untuk typecheck, `vitest` untuk suite.
- Tidak ada `unsafe`/FFI; deps runtime hanya kebun `@noble`/`@scure` (audit-zero-float).
- **Test:** `npm run typecheck` 0 error; `npm test` 28/28 lulus; `npm run build` sukses (ESM+CJS+DTS).

### SDK1 — Envelope 168-Byte Kanonikal
Layout byte eksak (urutan sama dengan `crates/aurion-ledger/src/codec.rs` → `encode_tx`):

| Offset | Panjang | Field         | Encoding        |
|--------|---------|---------------|-----------------|
| 0      | 8       | nonce         | `u64` LE        |
| 8      | 32      | sender        | pubkey ed25519  |
| 40     | 32      | recipient     | pubkey ed25519  |
| 72     | 16      | amount        | `u128` LE (Quanta) |
| 88     | 16      | fee           | `u128` LE (Quanta) |
| 104    | 64      | signature     | ed25519 (preimage = 104 byte pertama) |

- `buildSigningPayload` = 104 byte pertama (nonce|sender|recipient|amount|fee).
- `decodeEnvelope` **menolak panjang ≠ 168**; `encodeEnvelope` menolak signature ≠ 64.
- Invariant SDK1 diujikan: panjang eksak, round-trip byte-for-byte, offset kanonikal,
  tamper pada region amount → verifikasi signature gagal.
- **Test:** `tests/envelope.test.ts`.

### SDK2 — Ed25519, Mnemonic, Derivasi Alamat, & Preimage 104
- Keypair: `generate()`, `fromPrivateKey(hex|bytes)`, `fromMnemonic(BIP39 Inggris)`  → 32-byte priv/publ + `sign` (64 byte).
- `verify(sig, payload)` menolak signature panjang ≠ 64.
- `deriveAccountId = BLAKE3("AURION_ADDR_CANONICAL_V1" || pubkey)` (sama dengan
  `crates/aurion-account/src/address.rs`); `accountIdHex` = 64 karakter.
- Cross-check tanda tangan SDK terhadap verifikator independen (`@noble/ed25519` lokal;
  daftar kandidat selanjutnya: `ed25519-dalek` di Rust → integrasi `aurion-wallet`).
- **Test:** `tests/keypair.test.ts`.

### SDK3 — Transport Zenoh-Shim (HTTP) & TxReceipt
- `AurionTransport` interface: `getAccount(chainId, accountId)`, `submitTx(chainId, envelope)`.
- `HttpTransport` (fetch): `GET aurion/{chainId}/account/{pubkeyHex}` → `{ address, balance, nonce }`;
  `POST aurion/{chainId}/tx/submit` body `application/octet-stream` → `{ txHash }` dengan
  `txHash = BLAKE3(envelope)` pra-submit; timeout via `AbortSignal.timeout`.
- Jalur akun memakai **hex pubkey 32-byte** sebagai `accountId` (mengikuti parser
  `aurion-gateway`, bukan `accountIdHex` hasil derivasi).
- **Test:** tercakup pada kontrak `AurionTransport` + utas HttpTransport (belum live-e2e).

### SDK4 — Chip Moneter Quanta Zero-Float
- `Quanta.fromAur("0.1")` = `1_000_000_000n`; `"0.0000000001"` = `1n`; `QUANTA_PER_AUR = 10^10`.
- Parser menolak: tanda `+/-`, eksponen (`1e3`), `.5`, `5.`, spasi, `NaN`, presisi > 10 desimal.
- Arithmetic BigInt murni: `add/sub/mul(scalar)/divFloor` dengan proteksi **underflow**
  (`QuantaUnderflowError`); `mul(Quanta)` eksplisit `mulQuanta` (trap dimensional menghindari Quanta²).
- Nilai > `2^53-1` tidak pernah kehilangan presisi (round-trip eksak).
- **Test:** `tests/quanta.test.ts`.

### SDK5 — Konvensi Pemakaian & Determinisme
- `fee` **opsional** → default `DEFAULT_MIN_FEE_QUANTA = 1`; fee **0 ditolak fail-fast**.
- `amount` **wajib > 0**; `nonce` eksplisit → `NonceProvider` (mis. `+1` dari respon akun) → default `0`.
- Semua fungsi kodek/kriptografi pure; tidak ada state global mutabel di jalur pembayaran
  (kecuali pasokan hash sinkron noble yang di-inisialisasi sekali di `keypair.ts`).

---

## 4. CATATAN IMPLEMENTASI (DEVIASI & KEPUTUSAN)

1. **Canonical codec — deviasi dari draft inisial (PENTING).** Draft awal pekerja
   menentukan urutan/endian `nonce|fee|amount|sender|receiver` dengan integer **BE**.
   Ini bertentangan dengan kodeks kanonikal Rust
   (`crates/aurion-ledger/src/codec.rs`): **semua integer LE** dan urutan
   `nonce|sender|recipient|amount|fee`. SDK mengikuti **kanonikal Rust (LE)** agar
   envelope SDK **identik byte-demi-byte** dengan yang diverifikasi chain (invarian
   SDK2/SDK3). Draft pekerja direkam sebagai drafting error, bukan spesifikasi.
2. **`@noble/ed25519` v2.3 memerlukan wiring sinkron eksplisit.** Operasi sinkron
   `sign/getPublicKey/verify` menuntut `etc.sha512Sync` di-set (`@noble/hashes/sha512`).
   Tanpa ini legend `hashes.sha512Sync not set` muncul saat runtime.
3. **Trap dimensional `mul(Quanta)`.** Mengalikan dua Amount menampilkan Quanta²
   (`1 AUR × 0.5 AUR` menghasilan `5000000000` AUR). API memakai `mul(scalar bigint)` /
   `divFloor(divisor bigint)`; `mulQuanta` tersedia eksplisit bagi komputasi rasio/BPS.
4. **`fetch` `BodyInit` di Node types.** `Uint8Array<ArrayBufferLike>` tidak assignable ke
   `BodyInit`; body envelope dikonversi `toArrayBuffer` (salinan `ArrayBuffer`) sebelum POST.
5. **Rute akun memakai pubkey hex, bukan accountId derivasi.** Gateway mem-parse
   segmen jalur terakhir sebagai hex 32-byte pubkey; `deriveAccountId` tetap diekspor
   sebagai utilitas untuk klien yang membutuhkan ID berdaulat.
6. **Paket di luar cakupan guard scanner.** `tools/aurion_guard.py` meng-scan hanya
   `crates/`, `apps/`, `config/` dan berkas `.rs`; `packages/aurion-sdk` (TS) tidak
   dimintai linter Rust — disiplin zero-float dijaga oleh tes SDK4.
7. **`aurion-contract` DIHAPUS.** `crates/aurion-contract` (VM eksperimental) telah dihapus dari workspace; `aurion-channel` menjadi penggantinya.

---

## 5. VERIFIKASI

- `cd packages/aurion-sdk && npm run typecheck` → 0 error.
- `cd packages/aurion-sdk && npm test` → **3 files, 28 tests, 0 gagal** (SDK0–SDK5 invariant).
- `cd packages/aurion-sdk && npm run build` → ESM + CJS + DTS sukses.
- `python3 tools/aurion_guard.py check` → exit 0 (paket TS tidak di-scan, docs konsisten).

## 6. BACKLOG (FASE BERIKUTNYA)
- Transport live: `zenoh` (kanonik) + WebSocket/Browser untuk target browser.
- Wallet-tier inference (`aurion-wallet`): domain separation delegation, akses pada
  `RequestScope`, dan cross-verify signature SDK vs `ed25519-dalek` (integrasi Rust).
- Biometric / hardware-signer abstraction; pemaksaan `NonceProvider` default dari gateway.