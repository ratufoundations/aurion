# TASK: task-aurion-gateway
> **Target Modul:** `aurion-gateway` (`crates/aurion-gateway`)  
> **Kategori:** Batas Akses Eksternal, Sanitasi Ingress & Perutean CQRS (Boundary Ingress & CQRS Gateway Engine)  
> **Status:** COMPLETED  
> **Otoritas:** `Agents.md`, RFC-001 (Capability-Keeper)  
> **Tingkat Ketergantungan:** 3 (`aurion-core`, `aurion-ledger`, `aurion-mempool`, `aurion-criptografi`)

---

## 1. TUJUAN & FILOSOFI PENGUJIAN MODUL

Modul `aurion-gateway` adalah pintu gerbang utama (*perimeter boundary*) yang menghubungkan dunia luar (dompet pengguna, aplikasi Web3, CLI, node explorer, dan bot) dengan subsistem internal Aurion. Melalui antarmuka RPC (seperti JSON-RPC atau Zenoh boundary), gateway menangani sanitasi data masuk (*ingress sanitization*), pemisahan arsitektur *Command Query Responsibility Segregation* (CQRS), mitigasi serangan DoS, dan pembatasan laju (*rate limiting*).

Karena merupakan lapisan yang terpapar langsung ke jaringan publik tanpa proteksi tanda tangan validator, modul gateway menghadapi ancaman paling dinamis: injeksi data acak (*fuzzed/malformed payloads*), serangan penghabisan memori (*OOM via jumbo requests*), pembanjiran permintaan (*traffic flood*), serta kebocoran informasi internal (*internal state leakage*).

Tujuan task ini adalah **menguji, memvalidasi, dan mengunci perilaku `aurion-gateway` secara terisolasi** agar:
1. Memvalidasi dan menyaring setiap data mentah eksternal yang masuk sebelum didekode atau diteruskan ke subsistem inti (*Zero Malformed Leakage*).
2. Menegakkan pemisahan jalur **CQRS secara mutlak**:
   * **Command Write Path:** Pengiriman transaksi baru diarahkan murni ke `aurion-mempool` tanpa memblokir thread konsensus.
   * **Query Read Path:** Pembacaan saldo, blok, atau riwayat diarahkan langsung ke snapshot/indeks baca (`aurion-ledger`) tanpa menyentuh mekanisme penguncian tulis.
3. Menahan serangan kehabisan memori (*OOM DoS*) dengan menolak request yang melampaui batas ukuran maksimal (`MAX_REQUEST_SIZE`) sebelum payload dialokasikan ke memori heap secara masif.
4. Menerapkan pembatasan laju (*Rate Limiting*) berbasis algoritma *Token Bucket* deterministik integer `u64` per alamat pengirim atau identitas IP.
5. Menjaga isolasi kegagalan klien: request korup, pembatalan koneksi mendadak, atau timeout klien dilarang memicu *panic* (`unwrap`) atau membocorkan rincian galat internal/stack trace node (*error sanitization*).
6. Mengunci seluruh metrik volume transfer data, jumlah permintaan, latensi, dan kuota rate-limit murni pada representasi integer `u64` (*zero-float*).
7. Mematuhi disiplin arsitektur sistem: `#![forbid(unsafe_code)]`, zero-float, penolakan `.unwrap()` di jalur produksi, serta penanganan galat terstruktur melalui `thiserror`.

---

## 2. ARSITEKTUR CQRS BOUNDARY & INGRESS ROUTING

```text
       [Permintaan Klien Luar (Dompet / CLI / Explorer / DApp)]
                                 |
                                 ▼
       ┌────────────────────────────────────────────────────────┐
       │                 AURION INGRESS GATEWAY                 │
       │                                                        │
       │   [Filter 1: Batas Ukuran Request (Max Frame Guard)]   │
       │   [Filter 2: Token Bucket Rate Limiter (u64 / IP)]     │
       │   [Filter 3: Sanitasi Format Payload & Schema Guard]   │
       └────────────────────────────────────────────────────────┘
                                 |
                                 ▼
                     [Perutean Jalur CQRS]
                    /                     \
       (Command Write)                   (Query Read)
              |                                |
              ▼                                ▼
   ┌──────────────────────┐        ┌──────────────────────┐
   │ Ingress Write Path   │        │ Ingress Read Path    │
   │ - Validasi Pre-Check │        │ - Baca Snapshot      │
   │ - Teruskan ke Mempool│        │ - Query Balance/Tx   │
   └──────────────────────┘        └──────────────────────┘
              |                                |
              ▼                                ▼
     [aurion-mempool]                  [aurion-ledger]
  (SMR Ingestion Queue)             (Read-Only Snapshot Store)
```

---

## 3. SPESIFIKASI INVARIAN & MATRIKS UJI MODUL (GW0 – GW5)

Suite pengujian modul wajib menguji 6 dimensi invarian boundary gateway berikut:

### [GW0] Sanitasi Ingress & Penolakan Data Cacat (Ingress Sanitization & Anti-Malformed Injection)

* **Deskripsi:** Semua muatan data eksternal (JSON-RPC request, Zenoh binary query, raw hex) wajib divalidasi struktur dan skemanya. Masukan yang korup, representasi angka tidak wajar, atau byte liar wajib ditolak dengan respons galat standar tanpa memicu *panic* di gateway.
* **Kriteria Uji:**
* Request dengan format JSON cacat (tanda kurung kurawal tidak tertutup, karakter kontrol terlarang) ditolak dengan `Err(GatewayError::MalformedPayload)`.
* Injeksi bilangan negatif, float, atau representasi di luar presisi integer aman pada field `u128` Quanta (misal: `"amount": -100` atau `"amount": 50.5`) wajib ditolak seketika pada tahap parsing schema. Nilai moneter pada payload RPC diserialisasikan sebagai string desimal agar presisi >2^53 tetap terjaga di konsumen JSON/browser; response `account_get` menyajikan balance sebagai string.
* Karakter Unicode ilegal atau string heksadesimal ganjil pada `AccountId` ditolak dengan `Err(GatewayError::InvalidEncoding)`.
* Fuzzing stream: Membanjiri antarmuka gateway dengan 1.000 payload biner acak wajib menghasilkan respons galat terstruktur 100% tanpa crash atau `.unwrap()` failure.


### [GW1] Pemisahan Tegas Jalur CQRS (Strict CQRS Segregation: Write Command vs Read Query)

* **Deskripsi:** Jalur tulis (*Command/Ingress Write*) dan jalur baca (*Query/Ingress Read*) wajib terpisah secara arsitektural. Kueri baca dilarang menyentuh lock penulisan konsensus, dan penyerahan transaksi tulis dilarang melakukan operasi disk sinkron yang memblokir respons.
* **Kriteria Uji:**
* **Command Write (`send_transaction`):**
* Menerima transaksi tervalidasi tanda tangannya, menyalurkannya ke antarmuka `MempoolIngress`, lalu mengembalikan receipt/hash transaksi dalam waktu deterministik.
* Gagal menyalurkan ke mempool (misal mempool penuh) mengembalikan error spesifik tanpa mengunci thread eksekusi gateway.

* **Query Read (`get_balance`, `get_block_by_height`, `get_receipt`):**
* Dieksekusi langsung terhadap read-only snapshot store (`aurion-ledger`).
* Membuktikan bahwa pemanggilan 100 kueri baca paralel tidak menghambat antrean transaksi pada jalur tulis (*zero contention on write locks*).

* Percobaan menjalankan mutasi state melalui endpoint kueri baca wajib ditolak secara struktural pada level antarmuka tipe (*type-level read-only purity*).


### [GW2] Pertahanan Batas Ukuran & Anti-DoS Payload Raksasa (Max Request Size & Bounded Memory Defense)

* **Deskripsi:** Gateway wajib membatasi ukuran muatan request maksimal (`MAX_REQUEST_SIZE`, misal: 128 KB untuk transaksi tunggal, 2 MB untuk batch) pada tahap paling awal buffering HTTP/Zenoh sebelum dialokasikan ke struktur heap yang besar.
* **Kriteria Uji:**
* Kirim request dengan header mendeklarasikan ukuran normal, tetapi aliran body melebihi batas batas aman: gateway memutus aliran pada byte ke-`MAX_REQUEST_SIZE + 1` dan mengembalikan `Err(GatewayError::PayloadTooLarge { size, max_allowed })`.
* Kirim deklarasi ukuran biner raksasa (misal 500 MB atau `u32::MAX`): parser menolak secara instan tanpa mengalokasikan buffer seukuran angka tersebut (*Anti-OOM*).
* Pengujian verifikasi memori memastikan heap usage tidak melonjak signifikan saat banjir request raksasa disuntikkan.


### [GW3] Pembatasan Laju Nir-Pecahan Berbasis Token Bucket (Zero-Float Token Bucket Rate Limiting)

* **Deskripsi:** Gateway menerapkan sistem pembatasan laju permintaan (*Rate Limiter*) berbasis algoritma *Token Bucket* menggunakan integer murni `u64`. Penanda identitas (IP address atau `AccountId`) dibatasi konsumsi permintaannya per jendela waktu milidetik.
* **Kriteria Uji:**
* Konfigurasikan bucket: kapasitas 10 token, isi ulang 1 token per 100 ms.
* Kirim 10 request berturut-turut → seluruhnya berhasil (`Ok(())`).
* Kirim request ke-11 seketika → ditolak dengan `Err(GatewayError::RateLimitExceeded { retry_after_ms })`.
* Tunggu interval milidetik integer yang ditentukan → token terisi kembali dan request berikutnya disetujui.
* Seluruh perhitungan waktu, isi ulang (*replenishment*), dan kuota dihitung dengan `u64` ms tanpa tipe data `f32`/`f64`.


### [GW4] Isolasi Kegagalan Klien & Penutupan Galat Internal (Fault Isolation & Error Redaction)

* **Deskripsi:** Kesalahan fatal pada klien, pemutusan koneksi sepihak, atau galat internal pada database simpul tidak boleh membocorkan informasi privat sistem (*internal details*) kepada pemanggil eksternal.
* **Kriteria Uji:**
* Simulasikan galat internal pada level penyimpanan (misal disk I/O error atau thread panic yang tertangkap): gateway memetakan galat tersebut ke kode ramah publik `GatewayError::InternalError` dengan pesan umum dan request trace ID acak.
* Verifikasi bahwa pesan respons ke klien dilarang memuat path berkas sistem operasi (misal `/home/...`), nama tabel database, atau raw database error stack trace (*Zero Information Leakage*).
* Simulasikan klien yang memutuskan koneksi TCP saat gateway sedang memproses kueri: gateway membersihkan task secara bersih tanpa memicu *leaked task* atau *deadlock* pada worker pool.


### [GW5] Metrik Operasional & Kuota Ingress Nir-Pecahan (Zero-Float Gateway Ingress Metrics)

* **Deskripsi:** Semua metrik performa gateway—termasuk jumlah request diterima, total byte ingress/egress, waktu eksekusi request, dan counter penolakan—wajib dicatat murni menggunakan integer `u64`.
* **Kriteria Uji:**
* Pelacakan latensi request dicatat dalam bilangan bulat mikrodirektori atau milidetik (`u64` ms).
* Akumulasi throughput bandwidth ingress/egress menggunakan `checked_add` untuk mencegah *integer overflow*.
* Penghitungan rasio penolakan (*error rate BPS*) menggunakan basis poin:

$$\text{Rejection Rate BPS} = \frac{\text{rejected\_requests} \times 10.000}{\text{total\_requests}}$$ 

dihitung murni secara integer tanpa float.
* Audit statis: Tidak ada satupun token `f32` atau `f64` pada seluruh berkas sumber di dalam `crates/aurion-gateway/src/`.


---

## 4. ARSITEKTUR TEST SUITE & LOKASI FILE

Pengujian modul ini dikonsolidasikan sebagai suite integrasi internal modul `aurion-gateway`:

* **Target Modul:** `crates/aurion-gateway/`
* **Lokasi Suite Uji:** `crates/aurion-gateway/tests/gateway_invariants.rs`
* **Dependensi Pengujian (Dev-Dependencies):**
* `aurion-core`: Tipe `Transaction`, `Block`, `AccountId`, `Quanta`.
* `aurion-mempool`: Mock / in-memory instance untuk pengujian *Command Write Path*.
* `aurion-ledger`: Read-only snapshot store untuk pengujian *Query Read Path*.
* `aurion-criptografi`: Pembangkitan kunci pengirim untuk pembuatan request bertanda tangan sah.


---

## 5. KRITERIA PENYELESAIAN (DEFINITION OF DONE)

Task `task-aurion-gateway` dinyatakan selesai apabila:

1. Berkas spesifikasi tersimpan di `docs/task-register/TASK-aurion-gateway.md` dan terdaftar di `docs/STRUKTUR-FOLDER.txt`.
2. Seluruh matriks uji GW0–GW5 diimplementasikan di `crates/aurion-gateway/tests/gateway_invariants.rs` dan seluruhnya lulus:
```bash
cargo test -p aurion-gateway --test gateway_invariants

```

3. Seluruh unit test inline di dalam crate tetap lulus:
```bash
cargo test -p aurion-gateway

```

4. Lolos audit kepatuhan statis tanpa error dan warning:
```bash
cargo clippy -p aurion-gateway --all-targets --all-features -- -D warnings
python3 tools/aurion_guard.py check

```

5. Tidak ada penggunaan `unsafe`, tipe `f32`/`f64`, atau pemanggilan `.unwrap()` di jalur produksi `crates/aurion-gateway/src/`.
6. Ringkasan temuan dan konfirmasi status `COMPLETED` dicatat pada `logs/WORK_LOG.md`.
