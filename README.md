# Aurion

Aurion adalah proyek pembangunan ekosistem blockchain yang dikembangkan secara bertahap dari fondasi menuju lapisan aplikasi yang lebih tinggi.

## Architectural Authority

**`Agents.md` adalah direktif arsitektur dan sumber kebenaran utama proyek Aurion.**

Setiap pengembang, AI agent, automation, atau kontributor yang bekerja pada repository ini **wajib membaca dan memahami `Agents.md` sebelum melakukan perubahan yang menyentuh arsitektur, protokol, invariant, struktur modul, atau keputusan desain utama**.

Urutan otoritas:

```text
Agents.md
   ↓
Architecture / Protocol Decisions yang telah disetujui
   ↓
Task Register / Task Definitions
   ↓
Implementation
   ↓
Tests
```

### Aturan Prioritas

Jika terdapat konflik antara:

- README ini dan `Agents.md`
- kode dan `Agents.md`
- task dan `Agents.md`
- usulan agent dan keputusan arsitektur yang telah disetujui

maka **`Agents.md` memiliki prioritas**.

README ini hanya berfungsi sebagai orientasi proyek dan tidak boleh menjadi sumber aturan protokol yang berdiri sendiri.

## Agent and Contributor Boundary

AI agent dan automation dapat membantu implementasi, pengujian, audit, refactoring, dan pekerjaan teknis yang telah diberikan.

Namun agent **tidak memiliki kewenangan independen** untuk:

- mengubah `Agents.md`;
- mengubah keputusan arsitektur utama;
- mengubah protocol invariant;
- mengubah definisi atau acceptance criteria task;
- mengganti model protokol berdasarkan asumsi sendiri.

Jika implementasi menemukan konflik atau kebutuhan perubahan, perubahan tersebut harus **dilaporkan sebagai temuan dan diajukan sebagai usulan**, bukan diterapkan secara sepihak.

## Development Principle

Aurion dikembangkan secara bertahap.

Pada tahap fondasi, implementasi digunakan untuk membantu memperjelas pemahaman terhadap sistem. Tidak semua konsep harus langsung dikunci menjadi spesifikasi final.

Karena itu:

```text
Explore
   ↓
Build
   ↓
Test
   ↓
Observe
   ↓
Understand
   ↓
Refine
   ↓
Document
   ↓
Lock
```

Dokumentasi normatif hanya dikunci setelah keputusan desain benar-benar dipahami dan disetujui.

## Testing

Setiap modul Aurion akan dipetakan dan diuji secara bertahap.

Peta pengujian, task, dan status pekerjaan dicatat melalui **Task Register**.

Tes digunakan untuk memverifikasi perilaku aktual modul dan menjaga agar perubahan tidak merusak fondasi yang telah tervalidasi.

## Repository Structure

Struktur repository dibagi menjadi dua kelompok utama:

```text
crates/
    Core libraries and protocol modules

apps/
    Executable applications
```

Modul dapat berkembang atau berubah selama fase eksplorasi. Struktur repository saat ini **bukan dengan sendirinya merupakan spesifikasi final arsitektur**.

## Current Project State

Aurion saat ini berada pada **fase pembangunan dan pemetaan fondasi**.

Fokus utama fase ini adalah:

1. memahami setiap modul;
2. memetakan hubungan antar-modul;
3. membangun test map;
4. menemukan batas dan kebutuhan sistem;
5. mengevaluasi alternatif arsitektur;
6. mengunci keputusan hanya setelah cukup dipahami.

## Protocol Facts

Fakta protokol yang sudah dikunci dan menjadi acuan implementasi:

| Fakta | Nilai |
|---|---|
| Satuan moneter (`Quanta`) | `u128` (fixed-point integer, `zero-float`) |
| Presisi | 1 AUR = `10^10` Quanta (`QUANTA_PER_AUR`) |
| Denominator BPS | `10.000` (= 100%) |
| Pasokan genesis Treasury | 66.000.000 AUR = `660.000.000.000.000.000` Quanta (`TREASURY_GENESIS_QUANTA`) |
| Codec akun (ledger) | 24 byte (balance `u128` 16B + nonce `u64` 8B) |
| Codec transaksi | 168 byte fixed (payload 104 byte) |
| Serialisasi balance RPC | String desimal (presisi &gt; 2^53 aman bagi konsumen JSON/browser) |

Nilai kanonikal berada di `crates/aurion-core/src/types.rs`; konstanan lain boleh *re-export* tetapi tidak boleh menduplikasi nilai.
<br>Detail invariant pengujian diuraikan pada `docs/task-register/TASK-*.md`.

## Source of Truth

Untuk pekerjaan teknis:

> **Baca `Agents.md` terlebih dahulu.**

Untuk status dan pekerjaan:

> **Gunakan Task Register.**

Untuk perilaku yang harus dipertahankan:

> **Gunakan test dan invariant yang telah disetujui.**

---

**Aurion — Build the system, understand the system, then lock the rules.**
