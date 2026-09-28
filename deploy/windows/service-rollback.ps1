# Roll back to the specified backup.
param(
    [Parameter(Mandatory = $true)]
    [string]$Timestamp,

    [string]$ServiceName = "aicored",
    [string]$InstallDir = "$env:ProgramFiles\AICore"
)

$ErrorActionPreference = "Stop"

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "must run as Administrator"
    exit 1
}

$target = Join-Path $InstallDir "aicored.exe"
$backupDir = "$env:ProgramData\AICore\backups\$Timestamp"
$backup = "$backupDir\aicored.exe"

if (-not (Test-Path $backup)) {
    Write-Error "backup not found: $backup"
    Write-Host "available backups:"
    Get-ChildItem "$env:ProgramData\AICore\backups" -ErrorAction SilentlyContinue | Select-Object Name
    exit 1
}

Write-Host "stopping service"
Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

Copy-Item $backup $target -Force
Write-Host "restored from $backup"

sc.exe start $ServiceName | Out-Null
Start-Sleep -Seconds 3

$svc = Get-Service -Name $ServiceName
if ($svc.Status -eq "Running") {
    Write-Host "rollback complete"
} else {
    Write-Error "service is not running after rollback"
    exit 1
}
