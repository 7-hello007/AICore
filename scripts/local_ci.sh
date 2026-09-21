#!/usr/bin/env bash
set -euo pipefail

echo "==> fmt"
cargo fmt --all -- --check

echo "==> clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> build"
cargo build --workspace --all-targets

echo "==> unit"
cargo nextest run --workspace -E 'binary_id(/unit/)'

echo "==> contract"
cargo nextest run --workspace -E 'binary_id(/contract/)'

echo "==> integration"
cargo nextest run --workspace -E 'binary_id(/integration/)'

echo "==> smoke (10s)"
AICORE_SMOKE_SECONDS=10 cargo nextest run --workspace -E 'binary_id(/smoke/)'

echo "==> all green"

# Static source/package consistency gate.
"$(dirname "$0")/audit_v32_static.sh"
