# TASK-003: Peta Tes Fondasi Aurion v0 (Working Test Map)

* **Status Dokumen:** `WORKING-MAP` (bukan aturan arsitektur final)
* **Versi:** v0 — Fondasi
* **Cakupan:** seluruh workspace `crates/*` + `apps/*` di branch `main`
* **Dokumen Terkait:** `TASK-002-crypto-testing-standards.md` (`DRAFT`, kerangka C0–C7)
* **Catatan:** tidak ada `Agents.md` di `main`; dokumen ini adalah inventarisasi + rencana kerja, bukan klaim produk selesai.

---

## 0. Hasil Verifikasi Inventarisasi (Second-Pass, Terkonfirmasi Source)

> Dihitung dari marker `#[test]` / `#[tokio::test]` langsung di source (`crates/*`, `apps/*`).

| Modul | Tes Terlihat | Kondisi |
| --- | ---: | --- |
| `aurion-criptografi` | 3 | hash (1) + signature verifier (2) |
| `aurion-core` | 0 | belum ada unit test |
| `aurion-consensus` | 0 | belum ada unit test |
| `aurion-ledger` | 1 | persistence / atomic commit |
| `aurion-mempool` | 2 | fee / nonce / balance |
| `aurion-network` | 3 | codec (2) + TCP loopback (1) |
| `aurion-contract` | 3 | VM execution / rollback / gas |
| `aurion-account` | 1 | lifecycle |
| `aurion-genesis` | 1 | genesis initialization |
| `aurion-guard` | 3 | lib (1) + election (2) |
| `aurion-validator` | 1 | admission lifecycle |
| `aurion-gateway` | 1 | route formatting |
| `aurion-wallet` | 2 | pairing/delegation + tx generation |
| `aurion-node` | 0 | belum ada test |
| `aurion-cli` | 0 | belum ada test |
| `aurion-explorer` | 0 | belum ada test |
| **TOTAL** | **21** | inventarisasi source, bukan hasil run full suite |

---

## 1. Legenda Tingkat Uji (T0–T8)

| Kode | Nama | Arti Praktis |
| --- | --- | --- |
| T0 | Build / Lint / Static Integrity | `cargo check`, `clippy -D warnings`, `fmt --check`, `forbid(unsafe)`, zero-float |
| T1 | Unit Test | satu fungsi / satu struct, input valid |
| T2 | Boundary / Negative Test | batas panjang, nilai 0/maks, input korup wajib `Err`, tidak panic |
| T3 | Property / Invariant Test | ribuan input acak, relasi matematis tetap benar |
| T4 | Integration Test | dua modul atau lebih bertemu (mis. wallet → core → mempool) |
| T5 | Determinism Test | input sama → output sama, lintas thread / run |
| T6 | Persistence / Recovery Test | commit → mati → hidup → state sama |
| T7 | Concurrency / Fault Test | thread N, timeout, disconnect, duplicate, half-close |
| T8 | Resource / Stress Test | volume besar, gas habis, frame raksasa, replay N blok |

## 2. Legenda Status (cara mencentang)

| Status | Makna | Cara Centang |
| --- | --- | --- |
| `[ ] TODO` | belum dikerjakan | `- [ ]` dibiarkan kosong |
| `[x] DONE` | sudah lulus + ada log bukti | ganti menjadi `- [x]` + isi kolom Bukti |
| `INVENTORIED` | sudah dihitung dari source | tahap awal map ini |
| `APPROVED-NEXT` | boleh dipakai lapisan di atasnya | hanya setelah T yang disyaratkan DONE |

> Aturan bukti: setiap baris yang diklaim DONE wajib mengisi kolom **Bukti** dengan perintah + exit code + ringkasan (contoh: `cargo test -p aurion-core … ok 12 passed`).

## 3. Fase Fondasi (urutan kerja yang disarankan)

| Fase | Modul | Alasan |
| --- | --- | --- |
| **F0** | criptografi, core, ledger, consensus | fondasi terdalam; DSR bergantung pada determinism `State` |
| **F1** | mempool, network, genesis, validator | transaksi mengalir; node bisa genesis → mempool |
| **F2** | account, wallet, guard, gateway | identitas + boundary luar |
| **F3** | node, cli, explorer | integrasi ujung-ke-ujung + interface contract |
| **F4** | DSR research + execution | jalur paralel, tidak memblokir F0–F3; VM lama = baseline |

---

## 4. Peta Besar: Satu Tabel per Modul (bisa diperiksa statusnya)

> Kolom **St** memakai checkbox `- [ ]` / `- [x]` agar bisa dicentang langsung di Markdown.
> ID stabil: `TM-<MODUL>-<NNN>`.

### 4.1 `aurion-criptografi` — Prioritas: SANGAT TINGGI (F0, model disiplin; acuan C0–C7 TASK-002)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-CRYPTO-001 | - [x] INVENTORIED | T1 | `Hasher::digest` hash dasar | hash tidak kosong, deterministik | inventarisasi source `hash.rs` |
| TM-CRYPTO-002 | - [ ] TODO | T1 | KAT BLAKE3 official vectors | `assert_eq` byte-per-byte vs vektor resmi |  |
| TM-CRYPTO-003 | - [ ] TODO | T1 | KAT Ed25519 RFC 8032 | `assert_eq` byte-per-byte vs vektor RFC |  |
| TM-CRYPTO-004 | - [x] INVENTORIED | T1 | Sign → Verify valid | `Verify(pk,m,Sign(sk,m))=true` | inventarisasi `verifier.rs` (2 test) |
| TM-CRYPTO-005 | - [ ] TODO | T2 | Mutasi 1-bit pesan → ditolak | `Err`, bukan panic |  |
| TM-CRYPTO-006 | - [ ] TODO | T2 | Mutasi kunci publik → ditolak | `Err(InvalidPublicKey)` / verify gagal |  |
| TM-CRYPTO-007 | - [ ] TODO | T2 | Boundary panjang: 0/31/32/33/63/64/dll | panjang salah → `Err`, tidak panic |  |
| TM-CRYPTO-008 | - [ ] TODO | T2 | Kunci non-kanonikal → ditolak | `Err`, tidak panic |  |
| TM-CRYPTO-009 | - [ ] TODO | T1/T5 | Batch vs single identik | `batch == single` per item |  |
| TM-CRYPTO-010 | - [ ] TODO | T5 | Thread 1..N identik (rayon) | hasil sama untuk threads {1,2,4,8,16} |  |
| TM-CRYPTO-011 | - [ ] TODO | T3 | Properti streaming == one-shot | `StreamingHash(m)==OneShotHash(m)` |  |
| TM-CRYPTO-012 | - [ ] TODO | T7 | Fuzz parser kunci/signature | tanpa crash/panic/OOM |  |
| TM-CRYPTO-013 | - [ ] TODO | T4 | Differential vs reference | `Output_A==Output_B==Output_C` |  |

### 4.2 `aurion-core` — Prioritas: SANGAT TINGGI (F0, lubang terbesar; calon pusat DSR)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-CORE-001 | - [ ] TODO | T1 | `Transaction` canonical payload | byte layout stabil |  |
| TM-CORE-002 | - [ ] TODO | T5 | `Transaction::digest` determinism | `digest(t)==digest(t)` lintas run |  |
| TM-CORE-003 | - [ ] TODO | T1/T2 | Verifikasi signature valid | tx valid diterima |  |
| TM-CORE-004 | - [ ] TODO | T2 | Signature invalid / salah signer | ditolak |  |
| TM-CORE-005 | - [ ] TODO | T2 | Nonce: benar / mismatch / replay | hanya nonce tepat diterima |  |
| TM-CORE-006 | - [ ] TODO | T2 | Amount boundary: 0 / 1 / maks / overflow | aturan ekonomi ditegakkan |  |
| TM-CORE-007 | - [ ] TODO | T1 | `State` transfer valid | saldo pengirim/terima benar |  |
| TM-CORE-008 | - [ ] TODO | T2 | Sender hilang → `Err` | tidak panic |  |
| TM-CORE-009 | - [ ] TODO | T2 | Saldo kurang → `Err` | state tidak berubah |  |
| TM-CORE-010 | - [ ] TODO | T2 | Self-transfer | sesuai aturan (saldo/nonce konsisten) |  |
| TM-CORE-011 | - [ ] TODO | T2 | Recipient baru dibuat otomatis | akun baru muncul benar |  |
| TM-CORE-012 | - [ ] TODO | T5 | `S(t+1)=f(S(t),Tx)` deterministik | state sama + tx sama → state baru sama |  |
| TM-CORE-013 | - [ ] TODO | T5 | `state-root` determinism | root sama untuk urutan apply sama |  |
| TM-CORE-014 | - [ ] TODO | T1 | `BlockHeader::hash` stabil | hash header deterministik |  |
| TM-CORE-015 | - [ ] TODO | T2 | `tx_count` vs body mismatch → `Err` | header tidak bisa berbohong |  |
| TM-CORE-016 | - [ ] TODO | T1/T2 | `Block::execute` + `state_root` check | root salah → `StateRootMismatch` |  |

### 4.3 `aurion-consensus` — Prioritas: SANGAT TINGGI (F0, titik paling sensitif)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-CONS-001 | - [ ] TODO | T1 | `ValidatorSet`: 0 / 1 / N validator | konstruksi sesuai aturan |  |
| TM-CONS-002 | - [ ] TODO | T2 | Duplikat validator | ditolak / didedup sesuai spec |  |
| TM-CONS-003 | - [ ] TODO | T1 | Tabel `N → f → quorum` | formula yang dipakai terdokumentasi via test |  |
| TM-CONS-004 | - [ ] TODO | T2 | Di bawah / tepat / di atas threshold | hanya ≥quorum yang lolos |  |
| TM-CONS-005 | - [ ] TODO | T1 | Vote prevote vs precommit | tipe dibedakan benar |  |
| TM-CONS-006 | - [ ] TODO | T2 | Vote height/round salah | ditolak |  |
| TM-CONS-007 | - [ ] TODO | T2 | Signer bukan member / signature invalid | ditolak |  |
| TM-CONS-008 | - [ ] TODO | T2 | Duplikat vote | tidak dihitung ganda |  |
| TM-CONS-009 | - [ ] TODO | T2 | Conflicting vote (equivocation) | terdeteksi / ditolak sesuai aturan |  |
| TM-CONS-010 | - [ ] TODO | T1 | `RoundState` akumulasi → QC | QC terbit tepat saat quorum |  |
| TM-CONS-011 | - [ ] TODO | T5 | QC deterministik | input vote sama → QC sama |  |

### 4.4 `aurion-ledger` — Prioritas: TINGGI (F0)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-LEDGER-001 | - [x] INVENTORIED | T6 | Atomic commit + persistence dasar | `commit_block` → baca kembali sama | inventarisasi `store.rs` (1 test) |
| TM-LEDGER-002 | - [ ] TODO | T1 | Open / write / read by height | round-trip benar |  |
| TM-LEDGER-003 | - [ ] TODO | T1 | `latest_height` tracking | selalu menunjuk blok terakhir |  |
| TM-LEDGER-004 | - [ ] TODO | T1 | Account persistence | saldo/nonce tersimpan benar |  |
| TM-LEDGER-005 | - [ ] TODO | T2 | Duplicate commit height sama | ditolak / idempotent sesuai aturan |  |
| TM-LEDGER-006 | - [ ] TODO | T2 | Corrupt record → `Err` | tidak panic, error jelas |  |
| TM-LEDGER-007 | - [ ] TODO | T6 | Restart recovery: commit → close → reopen → read | state tetap sama |  |
| TM-LEDGER-008 | - [ ] TODO | T6/T8 | Multi-block replay N blok | seluruh N blok terbaca konsisten |  |

### 4.5 `aurion-mempool` — Prioritas: TINGGI (F1)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-MEMPOOL-001 | - [x] INVENTORIED | T1 | Insert valid + ordering dasar | tx valid masuk, urutan benar | inventarisasi `pool.rs` (2 test) |
| TM-MEMPOOL-002 | - [ ] TODO | T2 | Duplikat tx | ditolak kedua kali |  |
| TM-MEMPOOL-003 | - [ ] TODO | T2 | Nonce salah / gap | ditolak |  |
| TM-MEMPOOL-004 | - [ ] TODO | T2 | Saldo kurang / signature invalid | ditolak |  |
| TM-MEMPOOL-005 | - [ ] TODO | T2 | Amount 0 / maksimum | sesuai aturan fee/ekonomi |  |
| TM-MEMPOOL-006 | - [ ] TODO | T1 | Fee priority + nonce ordering | seleksi blok deterministik |  |
| TM-MEMPOOL-007 | - [ ] TODO | T1 | Block limit | tidak melebihi batas |  |
| TM-MEMPOOL-008 | - [ ] TODO | T4 | Commit cleanup vs state transition | committed terhapus, state konsisten |  |
| TM-MEMPOOL-009 | - [ ] TODO | T4 | Diuji terhadap state yang berubah | bukan state statis saja |  |

### 4.6 `aurion-network` — Prioritas: TINGGI (F1)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-NET-001 | - [x] INVENTORIED | T1 | Codec round-trip tiap varian | `decode(encode(m))==m` | inventarisasi `lib.rs` (2 test) |
| TM-NET-002 | - [x] INVENTORIED | T7 | TCP loopback | kirim → terima kembali | inventarisasi `lib.rs` (1 tokio test) |
| TM-NET-003 | - [ ] TODO | T2 | Malformed / truncated / oversized frame | `Err`, tidak panic |  |
| TM-NET-004 | - [ ] TODO | T2 | Magic / version invalid | ditolak |  |
| TM-NET-005 | - [ ] TODO | T1/T2 | Handshake: chain benar / salah | salah chain ditolak |  |
| TM-NET-006 | - [ ] TODO | T7 | Peer connect / disconnect / timeout / half-close | tidak bocor / tidak hang |  |
| TM-NET-007 | - [ ] TODO | T7 | Concurrent peer N | tetap benar |  |
| TM-NET-008 | - [ ] TODO | T4 | Vote A → network → node B → consensus | vote remote mencapai engine |  |

### 4.7 `aurion-genesis` — Prioritas: TINGGI (F1)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-GENESIS-001 | - [x] INVENTORIED | T1 | Genesis initialization dasar | blok 0 + state awal terbentuk | inventarisasi `lib.rs` (1 test) |
| TM-GENESIS-002 | - [ ] TODO | T2 | Spec tanpa validator / guard → `Err` | validasi menolak |  |
| TM-GENESIS-003 | - [ ] TODO | T2 | Chain-id / nama / timestamp invalid | ditolak |  |
| TM-GENESIS-004 | - [ ] TODO | T2 | AUR→Quantum: overflow / exact supply | tidak overflow diam-diam |  |
| TM-GENESIS-005 | - [ ] TODO | T6 | Idempotency + restart | genesis ulang hasil sama |  |
| TM-GENESIS-006 | - [ ] TODO | T5 | Spec sama → genesis sama; spec beda → commitment beda | deterministik |  |

### 4.8 `aurion-validator` — Prioritas: SEDANG-TINGGI (F1)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-VALID-001 | - [x] INVENTORIED | T1 | Admission lifecycle dasar | pending → active sesuai aturan | inventarisasi `lib.rs` (1 test) |
| TM-VALID-002 | - [ ] TODO | T2 | Registrasi duplikat / kunci invalid | ditolak |  |
| TM-VALID-003 | - [ ] TODO | T1 | Probation / endorsement / registry | transisi benar |  |
| TM-VALID-004 | - [ ] TODO | T2 | Transisi invalid | ditolak |  |
| TM-VALID-005 | - [ ] TODO | T5 | Registry deterministik | urutan / hasil stabil |  |

### 4.9 `aurion-account` — Prioritas: SEDANG (F2)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-ACCOUNT-001 | - [x] INVENTORIED | T1 | Lifecycle dasar | create → aktif benar | inventarisasi `lib.rs` (1 test) |
| TM-ACCOUNT-002 | - [ ] TODO | T1 | Role + policy + state | otorisasi sesuai peran |  |
| TM-ACCOUNT-003 | - [ ] TODO | T2 | Transisi invalid / duplikat device | ditolak |  |
| TM-ACCOUNT-004 | - [ ] TODO | T1/T2 | Revoke + recovery | revoke menutup akses, recovery membuka benar |  |

### 4.10 `aurion-wallet` — Prioritas: SEDANG (F2)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-WALLET-001 | - [x] INVENTORIED | T1 | Pairing/delegation + tx generation | tx terbentuk + ter-sign | inventarisasi `lib.rs` (2 test) |
| TM-WALLET-002 | - [ ] TODO | T1 | Key generation + persistence | kunci tersimpan, load kembali sama |  |
| TM-WALLET-003 | - [ ] TODO | T2 | Invalid tx dicegah di sisi wallet | tidak bisa buat tx cacat |  |
| TM-WALLET-004 | - [ ] TODO | T4 | wallet tx → core verify → mempool accept | rantai boundary lolos |  |

### 4.11 `aurion-guard` — Prioritas: SEDANG (F2)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-GUARD-001 | - [x] INVENTORIED | T1 | Council/verdict/blacklist dasar | keputusan tercatat benar | inventarisasi `lib.rs` (1 test) |
| TM-GUARD-002 | - [x] INVENTORIED | T1/T5 | Election 2 test | ranking + tie-break deterministik | inventarisasi `election.rs` (2 test) |
| TM-GUARD-003 | - [ ] TODO | T2 | Evidence invalid / kandidat ineligible | didiskualifikasi |  |
| TM-GUARD-004 | - [ ] TODO | T3 | Property tie-breaking | tidak ada hasil non-deterministik |  |
| TM-GUARD-005 | - [ ] TODO | T1/T2 | Pardon + state transition | pardon membuka sesuai aturan |  |

### 4.12 `aurion-gateway` — Prioritas: SEDANG (F2, boundary luar)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-GATEWAY-001 | - [x] INVENTORIED | T1 | Route formatting | string route kanonis | inventarisasi `lib.rs` (1 test) |
| TM-GATEWAY-002 | - [ ] TODO | T1 | Ser/der request + error mapping | skema stabil |  |
| TM-GATEWAY-003 | - [ ] TODO | T2 | Invalid parameter | `Err` jelas |  |
| TM-GATEWAY-004 | - [ ] TODO | T4 | Submit tx + baca state rantai | tembus ke mempool/ledger |  |

### 4.13 `aurion-node` — Prioritas: TINGGI (F3, lubang terbesar; milestone L1)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-NODE-001 | - [ ] TODO | T4 | Start → genesis → load state | node hidup dari genesis |  |
| TM-NODE-002 | - [ ] TODO | T4 | Receive tx → mempool → execution → block | blok terbentuk |  |
| TM-NODE-003 | - [ ] TODO | T4 | Consensus → commit → ledger | blok ter-commit |  |
| TM-NODE-004 | - [ ] TODO | T6 | Restart → state recovery | state sama setelah restart |  |

### 4.14 `aurion-cli` — Prioritas: RENDAH-SEDANG (F3, interface contract)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-CLI-001 | - [ ] TODO | T0/T1 | Command parsing + `--help` | semua subcommand terdaftar |  |
| TM-CLI-002 | - [ ] TODO | T2 | Invalid args → exit code ≠ 0 | pesan jelas, tidak panic |  |
| TM-CLI-003 | - [ ] TODO | T1 | `keygen` offline | hex 64-char pub/priv valid |  |
| TM-CLI-004 | - [ ] TODO | T2/T4 | `balance` alamat 64-hex vs invalid | invalid ditolak lokal |  |
| TM-CLI-005 | - [ ] TODO | T1 | Output formatting stabil | skema tabel tidak berubah diam-diam |  |

### 4.15 `aurion-explorer` — Prioritas: RENDAH (F3)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-EXPLORER-001 | - [ ] TODO | T1 | DTO mapping | field lengkap |  |
| TM-EXPLORER-002 | - [ ] TODO | T1/T2 | Handler query valid / missing block / missing account | 404/error jelas |  |
| TM-EXPLORER-003 | - [ ] TODO | T1 | Response schema stabil | kontrak API tidak berubah diam-diam |  |

### 4.16 `aurion-contract` — Posisi: BASELINE PENELITIAN (F4, jangan diperbesar dulu)

| ID | St | Tingkat | Target Uji | Kriteria Lulus | Bukti |
| --- | --- | --- | --- | --- | --- |
| TM-CONTRACT-001 | - [x] INVENTORIED | T1 | Counter execution | hasil eksekusi benar | inventarisasi `vm.rs` |
| TM-CONTRACT-002 | - [x] INVENTORIED | T1 | Rollback | state kembali | inventarisasi `vm.rs` |
| TM-CONTRACT-003 | - [x] INVENTORIED | T8 | Infinite loop / gas habis | berhenti via gas | inventarisasi `vm.rs` |
| TM-CONTRACT-004 | - [ ] TODO | F4 | DSR investigation (paralel) | kandidat arsitektur vs baseline VM |  |

> Status konseptual: VM lama = `research baseline`, DSR = `architecture candidate`. Jangan hapus VM sebelum DSR terbukti.

---

## 5. Tabel Status Ringkas per Modul (centang cepat)

| Modul | Fase | INV | T1 | T2 | T3 | T4 | T5 | T6 | T7 | T8 | Siap Lanjut |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| criptografi | F0 | - [x] | - [ ] | - [ ] | - [ ] | - [ ] | - [ ] | n/a | - [ ] | n/a | - [ ] |
| core | F0 | - [x] | - [ ] | - [ ] | n/a | n/a | - [ ] | n/a | n/a | n/a | - [ ] |
| ledger | F0 | - [x] | - [ ] | - [ ] | n/a | n/a | n/a | - [ ] | n/a | - [ ] | - [ ] |
| consensus | F0 | - [x] | - [ ] | - [ ] | n/a | n/a | - [ ] | n/a | n/a | n/a | - [ ] |
| mempool | F1 | - [x] | - [x] | - [ ] | n/a | - [ ] | n/a | n/a | n/a | n/a | - [ ] |
| network | F1 | - [x] | - [x] | - [ ] | n/a | - [ ] | n/a | n/a | - [ ] | - [ ] | - [ ] |
| genesis | F1 | - [x] | - [x] | - [ ] | n/a | n/a | - [ ] | - [ ] | n/a | n/a | - [ ] |
| validator | F1 | - [x] | - [x] | - [ ] | n/a | n/a | - [ ] | n/a | n/a | n/a | - [ ] |
| account | F2 | - [x] | - [x] | - [ ] | n/a | n/a | n/a | n/a | n/a | n/a | - [ ] |
| wallet | F2 | - [x] | - [x] | - [ ] | n/a | - [ ] | n/a | n/a | n/a | n/a | - [ ] |
| guard | F2 | - [x] | - [x] | - [ ] | - [ ] | n/a | n/a | n/a | n/a | n/a | - [ ] |
| gateway | F2 | - [x] | - [x] | - [ ] | n/a | - [ ] | n/a | n/a | n/a | n/a | - [ ] |
| node | F3 | - [x] | n/a | n/a | n/a | - [ ] | n/a | - [ ] | n/a | n/a | - [ ] |
| cli | F3 | - [x] | - [ ] | - [ ] | n/a | - [ ] | n/a | n/a | n/a | n/a | - [ ] |
| explorer | F3 | - [x] | - [ ] | - [ ] | n/a | n/a | n/a | n/a | n/a | n/a | - [ ] |
| contract | F4 | - [x] | - [x] | n/a | n/a | n/a | n/a | n/a | n/a | - [x] | baseline |

> `n/a` = tidak wajib untuk modul itu. `INV` = inventarisasi 21 test selesai di dokumen ini.

---

## 6. Langkah Pertama yang Disarankan (tidak sekaligus 100%)

1. Kerjakan **F0** dulu: `TM-CORE-*` (16 item) → `TM-CONS-*` (11 item) → `TM-LEDGER-002..008` → `TM-CRYPTO-002..013` (melengkapi C0–C7 TASK-002).
2. Setiap item DONE wajib ada bukti log; tanpa log, status tetap TODO.
3. Setelah F0 `Siap Lanjut` semua `[x]`, baru petakan detail F1.
4. DSR tetap jalur paralel (F4), tidak memblokir F0–F3.




