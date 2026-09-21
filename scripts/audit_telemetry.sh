#!/usr/bin/env bash
set -euo pipefail

echo "=== ComputerSnapshot 定义 ==="
grep -n "pub struct ComputerSnapshot" -A 25 contracts/src/telemetry/snapshot.rs

echo
echo "=== Linux Provider collect ==="
grep -n "fn collect" -A 5 adapters/linux/src/telemetry/provider.rs

echo
echo "=== Windows Provider collect ==="
grep -n "fn collect" -A 5 adapters/windows/src/telemetry/provider.rs

echo
echo "=== System::new / refresh 调用 ==="
grep -rn "System::new\|refresh_all\|refresh_cpu\|refresh_memory\|refresh_processes" \
  --include="*.rs" adapters/ | grep -v target || true

echo
echo "=== timestamp 用法 ==="
grep -rn "timestamp:" --include="*.rs" . | grep -v target | grep -v "unix" || true

echo
echo "=== SystemTime import ==="
grep -rn "use std::time::SystemTime" --include="*.rs" . | grep -v target || true
