#!/usr/bin/env bash
set -euo pipefail

echo "==> fmt"
cargo fmt --all -- --check

echo "==> clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "==> build"
cargo build --workspace --all-targets --locked

echo "==> complete test suite"
if cargo nextest --version >/dev/null 2>&1; then
    cargo nextest run --workspace --locked
else
    echo "cargo-nextest not installed; falling back to cargo test"
    cargo test --workspace --all-targets --locked
fi

echo "==> static V4 audit"
"$(dirname "$0")/audit_v4_static.sh"

echo "==> all green"
