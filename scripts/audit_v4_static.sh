#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
fail(){ echo "[FAIL] $*" >&2; exit 1; }
pass(){ echo "[PASS] $*"; }

grep -q 'version = "0.4.0"' Cargo.toml || fail "workspace version is not 0.4.0"
grep -q 'rust-version = "1.75"' Cargo.toml || fail "workspace rust-version missing"
grep -q 'fn decide_runtime_route' runtime/src/adaptive_loop.rs || fail "side-effect-free route decision missing"
grep -q 'execute_runtime_binding' runtime/src/adaptive_loop.rs || fail "post-validation binding missing"
grep -q 'HardwareDeviceKind::Gpu' runtime-router/src/binder.rs || fail "GPU capability kind check missing"
grep -q 'HardwareDeviceKind::Npu' runtime-router/src/binder.rs || fail "NPU capability kind check missing"
grep -q 'TaskState::Expired' task-runtime/src/queue.rs || fail "expired task state missing"
grep -q 'trusted plugin requires Ed25519 signature' plugin/src/trust.rs || fail "trusted plugin Ed25519 gate missing"
grep -q 'MAX_MESSAGE_BYTES' ipc/src/transport/mod.rs || fail "IPC message bound missing"

for f in \
 core/src/lib.rs.realPC \
 runtime/src/lib.rs.realPC \
 apps/aicored/src/runtime.rs.realPC \
 apps/aicored/src/scheduler.rs.realPC \
 apps/aicore-dashboard/src/client.r \
 tests/Cargo.toml.model.should.delete \
 aicored-once-output.txt; do
  [[ -e "$f" ]] || fail "required retained residual file missing: $f"
done

python3 - <<'PY'
from pathlib import Path
import re, tomllib, json
excluded={'.git','env','target'}
for p in Path('.').rglob('Cargo.toml'):
    if not excluded.intersection(p.parts):
        tomllib.loads(p.read_text(encoding='utf-8-sig'))
for p in Path('.').rglob('*.json'):
    if not excluded.intersection(p.parts):
        json.loads(p.read_text(encoding='utf-8-sig'))
files={p.name for p in Path('tests').glob('*.rs')}
paths=set(re.findall(r'^path\s*=\s*"([^"]+\.rs)"', Path('tests/Cargo.toml').read_text(encoding='utf-8-sig'), re.M))
support=set()
for p in Path('tests').glob('*.rs'):
    support.update(re.findall(r'#\[path\s*=\s*"([^"]+\.fixture\.rs)"\]',p.read_text(encoding='utf-8-sig')))
paths.update(support)
if files != paths:
    raise SystemExit(f'test manifest mismatch: undeclared={sorted(files-paths)} missing={sorted(paths-files)}')
print(f'[PASS] parsed Cargo.toml/JSON; test manifest complete ({len(files)} Rust test files)')
PY
pass "V4 static defect-correction invariants"
