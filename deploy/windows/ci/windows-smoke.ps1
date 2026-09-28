# Windows CI smoke test。
#
# Yes. GitHub Actions Windows Runner on:
#   Construction.
#   2. Run in --once mode, verify exit code 0.
#   3. The certification status document is written.
#   4. Build service-host crime.
#
# No real Windows Service (needs admin, CI runner is available but time-consuming).

$ErrorActionPreference = "Stop"

Write-Host "==> build workspace"
cargo build --workspace --all-targets
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Host "==> build service-host"
cargo build -p aicore-service-host
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Host "==> run aicored --once"
$statePath = Join-Path $env:TEMP "aicore-status.json"
$ipcPath = Join-Path $env:TEMP "aicore.sock"
Remove-Item $statePath -ErrorAction SilentlyContinue

& "$env:GITHUB_WORKSPACE\target\debug\aicored.exe" `
    --once `
    --state-path $statePath `
    --ipc-path $ipcPath `
    --memory-path "$env:TEMP\aicore-memory.db" `
    --plugins-dir "$env:TEMP\aicore-plugins"

if ($LASTEXITCODE -ne 0) {
    Write-Error "aicored --once failed"
    exit 1
}

Write-Host "==> verify state file"
if (-not (Test-Path $statePath)) {
    Write-Error "state file not written"
    exit 1
}

$state = Get-Content $statePath | ConvertFrom-Json
Write-Host "tick_count = $($state.tick_count)"
Write-Host "current_mode = $($state.current_mode)"

if ($state.tick_count -lt 1) {
    Write-Error "expected tick_count >= 1"
    exit 1
}

Write-Host "==> service-host tests"
cargo nextest run -p aicore-tests --test service_lifecycle_unit
if ($LASTEXITCODE -ne 0) { exit 1 }

cargo nextest run -p aicore-tests --test service_upgrade_integration
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Host "windows smoke: PASS"
