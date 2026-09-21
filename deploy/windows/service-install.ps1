# AICore Daemon — 正式 Windows Service 安装脚本。
#
# 用法：
#   .\service-install.ps1 -BinPath "C:\Program Files\AICore\aicored.exe"
#
# 行为：
#   1. 复制二进制到 Program Files。
#   2. 使用 sc.exe 创建 Windows Service。
#   3. 配置失败自动重启。
#   4. 启动服务。
#   5. 输出状态。
param(
    [Parameter(Mandatory = $true)]
    [string]$BinPath,

    [string]$InstallDir = "$env:ProgramFiles\AICore",
    [string]$ServiceName = "aicored",
    [string]$DisplayName = "AICore Daemon",
    [string]$Description = "AI Computer Control Layer"
)

$ErrorActionPreference = "Stop"

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Error "must run as Administrator"
    exit 1
}

if (-not (Test-Path $BinPath)) {
    Write-Error "binary not found: $BinPath"
    exit 1
}

# 1. 目录 + 复制
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$target = Join-Path $InstallDir "aicored.exe"
Copy-Item -Path $BinPath -Destination $target -Force
Write-Host "installed binary -> $target"

# 2. 停止旧服务（若存在）
$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "stopping existing service"
    Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    sc.exe delete $ServiceName | Out-Null
    Start-Sleep -Seconds 1
}

# 3. 创建服务
$configPath = "$env:ProgramData\AICore\config.json"
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $configPath) | Out-Null
if (-not (Test-Path $configPath)) {
    '{"version":"1.0","daemon":{"telemetry":{"sampling_interval_ms":100},"state_write_interval_ticks":10,"shutdown_timeout_ms":5000,"initial_mode":"normal"},"plugins_dir":null,"memory_path":null,"logging_level":"info","logging_json":true}' | Set-Content -Encoding UTF8 $configPath
}
$binArgs = "`"$target`" --config `"$configPath`" --log-format json"
$create = sc.exe create $ServiceName binPath= $binArgs start= auto DisplayName= $DisplayName
if ($LASTEXITCODE -ne 0) {
    Write-Error "sc create failed: $create"
    exit 1
}
Write-Host "service created"

# 4. 描述 + 失败恢复
sc.exe description $ServiceName $Description | Out-Null
sc.exe failure $ServiceName reset= 86400 actions= restart/5000/restart/5000/restart/5000 | Out-Null
Write-Host "recovery configured"

# 5. 启动
sc.exe start $ServiceName
Start-Sleep -Seconds 3

# 6. 状态
Get-Service -Name $ServiceName | Format-List Name, DisplayName, Status, StartType
