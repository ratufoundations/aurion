#!/usr/bin/env bash
set -euo pipefail

# Pewarnaan output terminal
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}============================================================${NC}"
echo -e "${BLUE}           AURION PROTOCOL - SECURITY AUDIT SUITE           ${NC}"
echo -e "${BLUE}============================================================${NC}"

# 1. Pastikan perkakas audit terpasang
ensure_tool() {
    local tool=$1
    local install_cmd=$2
    if ! command -v "$tool" &> /dev/null; then
        echo -e "${YELLOW}[SETUP] Memasang $tool...${NC}"
        eval "$install_cmd"
    fi
}

ensure_tool "cargo-audit" "cargo install cargo-audit --locked"
ensure_tool "cargo-deny" "cargo install cargo-deny --locked"

# 2. Pemeriksaan Statis: Larangan Kode Unsafe & Float via Clippy
echo -e "\n${BLUE}[STEP 1/4] Menjalankan Pemeriksaan Clippy & Larangan Unsafe/Float...${NC}"
cargo clippy --workspace --all-targets --all-features -- -D warnings
echo -e "${GREEN}✓ Clippy Lints: Lolos tanpa pelanggaran unsafe atau float.${NC}"

# 3. Verifikasi Database Kerentanan RustSec
echo -e "\n${BLUE}[STEP 2/4] Memeriksa Kerentanan Dependensi (RustSec Advisory)...${NC}"
cargo audit
echo -e "${GREEN}✓ Cargo Audit: Tidak ada celah keamanan terdeteksi.${NC}"

# 4. Verifikasi Lisensi & Kebijakan Dependensi (Cargo Deny)
echo -e "\n${BLUE}[STEP 3/4] Memeriksa Kepatuhan Lisensi & Sumber Crate (Cargo Deny)...${NC}"
cargo deny check
echo -e "${GREEN}✓ Cargo Deny: Seluruh lisensi dan sumber crate patuh standar.${NC}"

# 5. Pemindaian Regex Tambahan (Hard Guardrail)
echo -e "\n${BLUE}[STEP 4/4] Memindai codebase terhadap kata kunci dilarang (unsafe / float)...${NC}"
FAILED=0

# Pindai kemunculan blok unsafe di luar target pengujian
if grep -rn --exclude-dir={target,.git} --exclude="*.sh" "unsafe {" crates/ apps/; then
    echo -e "${RED}PELANGGARAN: Ditemukan blok unsafe eksplisit!${NC}"
    FAILED=1
fi

# Pindai deklarasi tipe f32 dan f64 langsung
if grep -rn --exclude-dir={target,.git} --exclude="*.sh" -E "(:|->)\s*(f32|f64)" crates/ apps/; then
    echo -e "${RED}PELANGGARAN: Ditemukan penggunaan tipe f32 atau f64!${NC}"
    FAILED=1
fi

if [ $FAILED -ne 0 ]; then
    echo -e "\n${RED}AUDIT GAGAL: Terdeteksi pelanggaran aturan fundamental!${NC}"
    exit 1
fi

echo -e "\n${GREEN}============================================================${NC}"
echo -e "${GREEN}      SEMUA PEMERIKSAAN KEAMANAN WORKSPACE BERHASIL         ${NC}"
echo -e "${GREEN}============================================================${NC}"
