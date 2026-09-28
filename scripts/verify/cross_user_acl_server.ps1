# Temporary simulated daemon for a different-login user's read-only pipe probe.
param([ValidateRange(30,300)][int]$Seconds=120)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$daemon=Join-Path $root 'target\debug\aicored.exe'
$cli=Join-Path $root 'target\debug\aicore.exe'
if (-not (Test-Path -LiteralPath $daemon) -or -not (Test-Path -LiteralPath $cli)) {
    throw 'Build the daemon and CLI in the desktop project before this probe.'
}
$folder=Join-Path $root ('env\acl-cross-user-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
New-Item -ItemType Directory -Force -Path $folder, (Join-Path $folder 'plugins') | Out-Null
$pipe='\\.\pipe\aicore-acl-'+[guid]::NewGuid().ToString('N')
$owner=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$originalMode=$env:AICORE_EXECUTION_MODE
$proc=$null
try {
    $env:AICORE_EXECUTION_MODE='simulated-platform'
    Write-Output "BEFORE target=pipe $pipe original=absent proposed=owner-only simulated daemon, ownerSID=$owner rollback=stop only daemon child; no system configuration changes"
    $arguments=@('--config',(Join-Path $folder 'config.json'),'--state-path',(Join-Path $folder 'state.json'),
      '--ipc-path',$pipe,'--memory-path',(Join-Path $folder 'memory.db'),
      '--plugins-dir',(Join-Path $folder 'plugins'),'--interval-ms','1000')
    $proc=Start-Process -FilePath $daemon -ArgumentList $arguments -WindowStyle Hidden -PassThru `
      -RedirectStandardOutput (Join-Path $folder 'daemon.stdout.log') `
      -RedirectStandardError (Join-Path $folder 'daemon.stderr.log')
    $ready=$false
    $missing=Join-Path $folder 'absent.json'
    for ($i=0;$i -lt 30;$i++) {
      if ($proc.HasExited) { break }
      & $cli status --ipc-path $pipe --state-path $missing *> $null
      if ($LASTEXITCODE -eq 0) {$ready=$true;break}
      Start-Sleep -Milliseconds 250
    }
    if (-not $ready) {throw 'Owner client could not connect to the protected pipe'}
    Write-Output "PASS owner connection. For OTHER LOGIN use: & '$root\scripts\verify\cross_user_acl_client.ps1' -PipeName '$pipe' -OwnerSid '$owner'"
    Write-Output "Pipe=$pipe OwnerSid=$owner WindowSeconds=$Seconds"
    Start-Sleep -Seconds $Seconds
}
finally {
    if ($proc -and -not $proc.HasExited) {
      Write-Output "NATIVE RESTORE target=owned daemon PID=$($proc.Id) original=no child proposed=stop child rollback=Stop-Process -Id $($proc.Id) -Force"
      Stop-Process -Id $proc.Id -Force
      $proc.WaitForExit()
    }
    $env:AICORE_EXECUTION_MODE=$originalMode
    if ($proc -and -not $proc.HasExited) { throw 'RESTORATION FAIL test daemon still running' }
    Write-Output "RESTORATION PASS owned daemon exited; mode restored; generated evidence=$folder"
}
