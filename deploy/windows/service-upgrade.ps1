# Upgrade AICore Windows Service (with automatic rollback).
param(
    [Parameter(Mandatory = $true)]
    [string]$NewBinPath,

    [string]$ServiceName = "aicored",
    [string]$InstallDir = "$env:ProgramFiles\AICore"
)

$ErrorActionPreference = "Stop"

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "must run as Administrator"
    exit 1
}

$target = Join-Path $InstallDir "aicored.exe"
$backup = "$target.bak"
$timestamp = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
$backupDir = "$env:ProgramData\AICore\backups\$timestamp"

if (-not (Test-Path $target)) {
    Write-Error "existing binary not found: $target"
    exit 1
}

# 1. Backup
New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
Copy-Item $target "$backupDir\aicored.exe" -Force
Write-Host "backed up -> $backupDir"

# 2. Closure of services
Write-Host "stopping service"
Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

# 3. Replacement binary
try {
    Copy-Item $NewBinPath $target -Force
    Write-Host "binary replaced"
} catch {
    Write-Error "copy failed: $_"
    Copy-Item "$backupDir\aicored.exe" $target -Force
    Start-Service -Name $ServiceName
    exit 1
}

# Start
sc.exe start $ServiceName | Out-Null
Start-Sleep -Seconds 3

# 5. Health screening
$svc = Get-Service -Name $ServiceName
if ($svc.Status -ne "Running") {
    Write-Warning "health check failed; rolling back"
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Copy-Item "$backupDir\aicored.exe" $target -Force
    Start-Service -Name $ServiceName
    Write-Host "rolled back to $backupDir"
    exit 1
}

Write-Host "upgrade complete; backup kept at $backupDir"
