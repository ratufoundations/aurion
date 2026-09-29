#!/usr/bin/env python3
"""AURION PROTOCOL — HARD SECURITY GATEKEEPER & COMPILER FIREWALL
Otoritas: Agents.md
Lokasi  : tools/aurion_guard.py
Fungsi  : Memindai codebase Rust terhadap pelanggaran aturan determinisme
          dan memblokir eksekusi cargo jika terdeteksi anomali.
"""

import os
import re
import sys
import subprocess
from pathlib import Path
from typing import List

# ==========================================
# KONFIGURASI WARNA TERMINAL (ANSI)
# ==========================================
RED = "\033[1;31m"
GREEN = "\033[1;32m"
YELLOW = "\033[1;33m"
CYAN = "\033[1;36m"
BOLD = "\033[1m"
RESET = "\033[0m"

# ==========================================
# DIREKTORI & TARGET PEMINDAIAN
# ==========================================
# Root repositori berada satu tingkat di atas direktori tools/
WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
SCAN_DIRS = ["crates", "apps", "config"]
IGNORE_DIRS = {"target", ".git", ".cargo"}

# ==========================================
# DEFINISI REGEX PELANGGARAN ATURAN
# ==========================================
# 1. Larangan Mutlak Unsafe
RE_UNSAFE_BLOCK = re.compile(r"\bunsafe\s*(\{|\bfn\b|\btrait\b|\bimpl\b)")

# 2. Larangan Mutlak Tipe Floating-Point
RE_FLOAT_TYPES = re.compile(r"(:\s*|\b(as)\s+|\b(type)\s+\w+\s*=\s*)(f32|f64)\b")
RE_FLOAT_LITERAL = re.compile(r"\b\d+\.\d+(_?\d+)*(f32|f64)?\b")

# 3. Larangan unwrap() di jalur produksi (kecuali modul test)
RE_UNWRAP = re.compile(r"\.unwrap\(\)")

# 4. Larangan println! / eprintln! di crates pustaka murni
RE_PRINTLN = re.compile(r"\b(println!|eprintln!)\b")
RE_RUST_STRING = re.compile(r'"(?:\\.|[^"\\])*"')


class Violation:
    def __init__(self, file_path: Path, line_no: int, rule: str, snippet: str):
        self.file_path = file_path
        self.line_no = line_no
        self.rule = rule
        self.snippet = snippet.strip()

    def __str__(self):
        rel_path = self.file_path.relative_to(WORKSPACE_ROOT)
        return f"{RED}[DITOLAK]{RESET} {BOLD}{rel_path}:{self.line_no}{RESET}\n" \
               f"   {YELLOW}Aturan  :{RESET} {self.rule}\n" \
               f"   {CYAN}Snippet :{RESET} {self.snippet}"


class AurionGuard:
    def __init__(self):
        self.violations: List[Violation] = []

    def scan_file(self, file_path: Path):
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                lines = f.readlines()
        except Exception as e:
            print(f"{YELLOW}[WARN] Gagal membaca berkas {file_path}: {e}{RESET}")
            return

        is_crate = "crates" in file_path.parts
        is_entrypoint = file_path.name in ("lib.rs", "main.rs")
        has_forbid_unsafe = False
        in_test_module = False

        for i, line in enumerate(lines, start=1):
            stripped = line.strip()

            # Deteksi modul pengujian #[cfg(test)]
            if "#[cfg(test)]" in stripped:
                in_test_module = True

            # Abaikan komentar dan singkirkan string agar IP/versi/contoh angka tidak
            # salah diklasifikasikan sebagai literal floating-point Rust.
            if stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*"):
                continue
            code_line = re.sub(r"/\*.*?\*/", "", RE_RUST_STRING.sub("", line)).split("//", 1)[0]

            # Periksa deklarasi wajib #![forbid(unsafe_code)] di entrypoint
            if is_entrypoint and "#![forbid(unsafe_code)]" in stripped:
                has_forbid_unsafe = True

            # 1. ATURAN: Larangan blok unsafe
            if RE_UNSAFE_BLOCK.search(code_line):
                self.violations.append(
                    Violation(file_path, i, "BLOK UNSAFE DILARANG (Agents.md §1.1)", line)
                )

            # 2. ATURAN: Larangan tipe & literal pecahan (f32 / f64)
            if RE_FLOAT_TYPES.search(code_line) or (not in_test_module and RE_FLOAT_LITERAL.search(code_line)):
                self.violations.append(
                    Violation(file_path, i, "TIPE/LITERAL FLOAT DILARANG (Agents.md §1.2 - Gunakan u64 Quanta)", line)
                )

            # 3. ATURAN: Larangan unwrap() di luar modul tes
            if not in_test_module and RE_UNWRAP.search(line):
                self.violations.append(
                    Violation(file_path, i, "PENGGUNAAN .unwrap() DILARANG (Gunakan '?' atau Result)", line)
                )

            # 4. ATURAN: Larangan println! di dalam crates/
            if is_crate and not in_test_module and RE_PRINTLN.search(code_line):
                self.violations.append(
                    Violation(file_path, i, "PRINTLN! DI CRATE DILARANG (Wajib gunakan tracing::{info, debug})", line)
                )

        # Validasi header #![forbid(unsafe_code)] pada lib.rs dan main.rs
        if is_entrypoint and not has_forbid_unsafe:
            self.violations.append(
                Violation(file_path, 1, "HEADER WAJIB HILANG: Tambahkan '#![forbid(unsafe_code)]' di baris pertama", "")
            )

    def run_scanner(self) -> bool:
        print(f"{CYAN}=== AURION PROTOCOL SECURITY GUARD SCANNER ==={RESET}")
        print(f"Target Root: {WORKSPACE_ROOT}\n")
        self.violations.clear()

        for target in SCAN_DIRS:
            target_path = WORKSPACE_ROOT / target
            if not target_path.exists():
                continue

            for root, dirs, files in os.walk(target_path):
                dirs[:] = [d for d in dirs if d not in IGNORE_DIRS]
                for file in files:
                    if file.endswith(".rs"):
                        self.scan_file(Path(root) / file)

        if self.violations:
            print(f"\n{RED}{BOLD}🚨 TERDETEKSI {len(self.violations)} PELANGGARAN KEAMANAN:{RESET}\n")
            for v in self.violations:
                print(v)
                print("-" * 60)
            print(f"\n{RED}{BOLD}[BLOCKED] KOMPILASI DIBATALKAN KARENA KODE TIDAK MEMATUHI AGENTS.MD!{RESET}\n")
            return False

        print(f"{GREEN}✓ Seluruh modul patuh aturan: Zero-Unsafe, Zero-Float, Anti-Unwrap.{RESET}\n")
        return True


def install_git_hook():
    """Pasang tools/aurion_guard.py sebagai Git pre-commit hook otomatis"""
    git_hook_dir = WORKSPACE_ROOT / ".git" / "hooks"
    if not git_hook_dir.exists():
        print(f"{RED}Direktori .git/hooks tidak ditemukan. Pastikan repositori sudah di-init git.{RESET}")
        return

    pre_commit_path = git_hook_dir / "pre-commit"
    hook_content = """#!/usr/bin/env bash
python3 tools/aurion_guard.py check
"""
    with open(pre_commit_path, "w", encoding="utf-8") as f:
        f.write(hook_content)

    os.chmod(pre_commit_path, 0o755)
    print(f"{GREEN}✓ Git pre-commit hook berhasil dipasang di {pre_commit_path}{RESET}")


def main():
    guard = AurionGuard()

    # 1. Mode Pemeriksaan Statis
    if len(sys.argv) == 1 or sys.argv[1] == "check":
        success = guard.run_scanner()
        sys.exit(0 if success else 1)

    # 2. Mode Pasang Hook Git
    elif sys.argv[1] == "install-hook":
        install_git_hook()
        sys.exit(0)

    # 3. Mode Wrapper Cargo
    elif sys.argv[1] == "cargo":
        cargo_args = sys.argv[2:]
        if not guard.run_scanner():
            sys.exit(1)

        print(f"{CYAN}Guard Lolos. Menjalankan: cargo {' '.join(cargo_args)}{RESET}\n")
        cmd = ["cargo"] + cargo_args
        res = subprocess.run(cmd, cwd=WORKSPACE_ROOT)
        sys.exit(res.returncode)

    else:
        print("Penggunaan:")
        print("  python3 tools/aurion_guard.py check          : Pindai seluruh kode Rust")
        print("  python3 tools/aurion_guard.py install-hook   : Pasang sebagai git pre-commit hook")
        print("  python3 tools/aurion_guard.py cargo <args>   : Jalankan cargo hanya jika lolos audit")
        sys.exit(1)


if __name__ == "__main__":
    main()

