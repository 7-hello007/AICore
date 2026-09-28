# AICore daemon development mode start script (Windows).
#
# Usage:
#   . \install.ps1# Installed to %LOCALAPPDATA%AICore and started
#   . install.ps1-Stop # Stop
#   .\install.ps1 -Uninstall
param(
    [switch]$Stop,
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\AICore"
$StateDir   = "$env:LOCALAPPDATA\AICore\state"
$LogFile    = "$StateDir\aicored.log"

function Stop-Aicore {
    Get-Process -Name "aicored" -ErrorAction SilentlyContinue | Stop-Process -Force
}

if ($Stop) {
    Stop-Aicore
    Write-Host "aicored stopped."
    exit 0
}

if ($Uninstall) {
    Stop-Aicore
    Remove-Item -Recurse -Force $InstallDir -ErrorAction SilentlyContinue
    Write-Host "aicored uninstalled."
    exit 0
}

# Install
New-Item -ItemType Directory -Force -Path $InstallDir, $StateDir | Out-Null

$srcDir = Split-Path -Parent $PSCommandPath
$srcDir = Join-Path $srcDir "bin"
if (-not (Test-Path (Join-Path $srcDir "aicored.exe"))) {
    Write-Error "missing $srcDir\aicored.exe"
    exit 1
}

Copy-Item (Join-Path $srcDir "aicored.exe") $InstallDir -Force
Copy-Item (Join-Path $srcDir "aicore.exe") $InstallDir -Force -ErrorAction SilentlyContinue

Stop-Aicore
Start-Process -FilePath (Join-Path $InstallDir "aicored.exe") `
    -ArgumentList "--interval-ms 100 --state-path `"$StateDir\status.json`"" `
    -RedirectStandardOutput $LogFile `
    -RedirectStandardError "$LogFile.err" `
    -WindowStyle Hidden

Write-Host "aicored started; log: $LogFile"
