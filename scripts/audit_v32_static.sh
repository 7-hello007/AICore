#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

fail(){ echo "[FAIL] $1" >&2; exit 1; }
pass(){ echo "[PASS] $1"; }

[ -f Cargo.toml ] || fail "Cargo.toml missing"
[ -f Cargo.lock ] || fail "Cargo.lock missing"
[ -d docs ] || fail "docs missing"
[ ! -d destination ] || fail "destination must not be shipped"
[ -f LICENSE ] || fail "LICENSE missing"

# No release docs may point at version-specific runtime docs.
if grep -RInE 'docs/v[0-9]+/' docs >/tmp/aicore_v32_doc_refs.txt 2>/dev/null; then
  cat /tmp/aicore_v32_doc_refs.txt >&2
  fail "stale version/destination references in docs"
fi

# No fake ONNX placeholder in release tree.
find models -type f -name '*.placeholder' -print | grep -q . && fail "placeholder model artifact present" || true

# Tests must remain flat.
find tests -mindepth 2 -type f -print | grep -q . && fail "tests contains nested files" || true

pass "static V3.2 structure checks"
