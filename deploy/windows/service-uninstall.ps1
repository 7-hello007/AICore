# 停止并卸载 Windows Service。
param(
    [string]$ServiceName = "aicored",
    [switch]$Purge
)

$ErrorActionPreference = "Stop"

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "must run as Administrator"
    exit 1
}

$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    Write-Host "stopping service"
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    sc.exe delete $ServiceName | Out-Null
    Write-Host "service removed"
} else {
    Write-Host "service not installed"
}

if ($Purge) {
    $installDir = "$env:ProgramFiles\AICore"
    $stateDir = "$env:ProgramData\AICore"
    Remove-Item -Recurse -Force $installDir -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $stateDir -ErrorAction SilentlyContinue
    Write-Host "purged install + state"
}
