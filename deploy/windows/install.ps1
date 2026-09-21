# AICore daemon 开发模式启动脚本（Windows）。
#
# 用法：
#   .\install.ps1               # 安装到 %LOCALAPPDATA%\AICore 并启动
#   .\install.ps1 -Stop         # 停止
#   .\install.ps1 -Uninstall    # 停止并移除
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

# 安装
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
