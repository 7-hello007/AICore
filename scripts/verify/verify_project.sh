#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

pass(){ printf '[PASS] %s\n' "$1"; }
fail(){ printf '[FAIL] %s\n' "$1" >&2; exit 1; }

[[ -f Cargo.toml ]] || fail 'workspace Cargo.toml missing'
[[ -f Cargo.lock ]] || fail 'Cargo.lock missing'
[[ -f LICENSE ]] || fail 'LICENSE missing'
[[ -f docs/README.md ]] || fail 'global docs missing'
[[ ! -d destination ]] || fail 'destination/ must not be shipped in the runtime project package'
pass 'repository structure'

if command -v cargo >/dev/null 2>&1; then
  cargo check --workspace
  cargo test --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  pass 'cargo validation'
else
  printf '[WARN] cargo not installed; source-only structural validation completed.\n'
fi
