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
    cargo deny check
else
    printf '%s\n' '[INFO] cargo-deny is not installed; skipping licenses, bans, and source checks.'
fi

if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit
else
    printf '%s\n' '[INFO] cargo-audit is not installed; skipping RustSec advisory checks.'
fi

printf '\n%s\n' '✓ Aurion workspace audit passed.'
