#!/usr/bin/env bash
set -euo pipefail

echo "=== workflows ==="
ls -la .github/workflows/ 2>/dev/null || echo "none"

echo
echo "=== key sections of ci.yml ==="
grep -nE "nextest|cargo test|AICORE_SMOKE|matrix|clippy|fmt" .github/workflows/ci.yml 2>/dev/null || echo "no ci.yml"

echo
echo "=== [[test]] entries in tests/Cargo.toml ==="
grep -E '^name = ' tests/Cargo.toml 2>/dev/null | wc -l

echo
echo "=== test counts by type ==="
for k in unit contract integration smoke; do
  n=$(grep -cE "_${k}\"" tests/Cargo.toml 2>/dev/null || echo 0)
  echo "${k}: ${n}"
done
