#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' '=== 1. Aurion security guard (crates/, apps/, config/) ==='
python3 tools/aurion_guard.py check

printf '\n%s\n' '=== 2. Formatting and strict Clippy ==='
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings

printf '\n%s\n' '=== 3. Workspace tests ==='
cargo test --workspace

printf '\n%s\n' '=== 4. Optional supply-chain checks ==='
if command -v cargo-deny >/dev/null 2>&1; then
    # Level lint advisori dikendalikan lewat CLI (cargo-deny >= 0.20).
    # CATATAN: jangan menambah `--deny=vulnerability` — flag CLI macam itu
    # menimpa daftar `ignore` [advisories] di deny.toml. Default vuln/yanked
    # sudah `deny`; hanya `unmaintained` diturunkan ke `warn`.
    cargo deny check --warn=unmaintained
else
    printf '%s\n' '[INFO] cargo-deny is not installed; skipping licenses, bans, source, and RustSec checks.'
fi

if command -v cargo-audit >/dev/null 2>&1; then
    # RustSec independen (basis data terpisah); pengecualian pada
    # `.cargo/audit.toml` sinkron dengan `deny.toml`.
    cargo audit
else
    printf '%s\n' '[INFO] cargo-audit is not installed; skipping standalone RustSec audit.'
fi

printf '\n%s\n' '✓ Aurion workspace audit passed.'
