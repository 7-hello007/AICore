#!/usr/bin/env bash
set -euo pipefail

# Linux-only: this fixture was captured through the read-only Linux adapters.
# The daemon's temp, migration, IPC, plugin and SQLite paths are all isolated.
cd "$(dirname "$0")/../.."
scratch="$(mktemp -d "${TMPDIR:-/tmp}/aicore-platform-simulator.XXXXXX")"
mkdir -p "$scratch/aicore"

cargo build -p aicore-daemon --locked

run_daemon() {
  AICORE_EXECUTION_MODE=simulated-platform TMPDIR="$scratch" \
    target/debug/aicored \
      --config "$scratch/config.json" \
      --state-path "$scratch/$1-status.json" \
      --ipc-path "$scratch/$1.sock" \
      --memory-path "$scratch/$1-memory.db" \
      --plugins-dir "$scratch/plugins" \
      --interval-ms 100 \
      --log-format json "${@:2}"
}

run_daemon once --once > "$scratch/once.log"
AICORE_EXECUTION_MODE=simulated-platform TMPDIR="$scratch" \
  timeout --preserve-status -s INT 3s \
  target/debug/aicored \
    --config "$scratch/config.json" \
    --state-path "$scratch/continuous-status.json" \
    --ipc-path "$scratch/continuous.sock" \
    --memory-path "$scratch/continuous-memory.db" \
    --plugins-dir "$scratch/plugins" \
    --interval-ms 100 \
    --log-format json > "$scratch/continuous.log"

python3 - "$scratch" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
once = json.loads((root / "once-status.json").read_text())
continuous = json.loads((root / "continuous-status.json").read_text())
assert once["tick_count"] == 1
assert continuous["tick_count"] >= 2
assert continuous["successful_ticks"] >= 1
assert continuous["failed_ticks"] == 0
log = (root / "continuous.log").read_text()
assert '"platform":"simulated_platform"' in log
print(f"simulated platform PASS: {continuous['tick_count']} ticks, "
      f"{continuous['successful_ticks']} successful, "
      f"{continuous['failed_ticks']} failed; artifacts={root}")
PY
