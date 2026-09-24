# Runs only project-built binaries in simulated Platform Mode. No host setters.
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$daemon = Join-Path $projectRoot 'target\debug\aicored.exe'
$cli = Join-Path $projectRoot 'target\debug\aicore.exe'
if (-not (Test-Path -LiteralPath $daemon) -or -not (Test-Path -LiteralPath $cli)) {
    throw 'Build aicore-daemon and aicore-cli before running this smoke test.'
}

$scratch = Join-Path $projectRoot ('env\windows-smoke-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
New-Item -ItemType Directory -Force -Path $scratch, (Join-Path $scratch 'plugins') | Out-Null
$pipe = '\\.\pipe\aicore-simulator-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$statusPath = Join-Path $scratch 'status.json'
$oldMode = $env:AICORE_EXECUTION_MODE
$oldTemp = $env:TEMP
$oldTmp = $env:TMP
$env:AICORE_EXECUTION_MODE = 'simulated-platform'
$env:TEMP = $scratch
$env:TMP = $scratch
$process = $null
try {
    $arguments = @(
        '--config', (Join-Path $scratch 'config.json'),
        '--state-path', $statusPath,
        '--ipc-path', $pipe,
        '--memory-path', (Join-Path $scratch 'memory.db'),
        '--plugins-dir', (Join-Path $scratch 'plugins'),
        '--interval-ms', '100',
        '--log-format', 'json'
    )
    $process = Start-Process -FilePath $daemon -ArgumentList $arguments `
        -RedirectStandardOutput (Join-Path $scratch 'daemon.stdout.log') `
        -RedirectStandardError (Join-Path $scratch 'daemon.stderr.log') `
        -WindowStyle Hidden -PassThru

    $state = $null
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        Start-Sleep -Milliseconds 100
        if (Test-Path -LiteralPath $statusPath) {
            try { $state = Get-Content -Raw -LiteralPath $statusPath | ConvertFrom-Json } catch { }
            if ($state -and $state.tick_count -ge 2) { break }
        }
        if ($process.HasExited) { break }
    }
    if ($process.HasExited -or -not $state -or $state.tick_count -lt 2) {
        throw 'Simulated Windows daemon did not produce two ticks.'
    }
    if ($state.failed_ticks -ne 0 -or $state.successful_ticks -lt 1) {
        throw 'Simulated Windows daemon reported no successful memory-only execution or a failed tick.'
    }
    # A missing fallback file makes CLI success evidence of the named pipe.
    $missingFallback = Join-Path $scratch 'no-fallback.json'
    if (Test-Path -LiteralPath $missingFallback) { throw 'Fallback sentinel unexpectedly exists.' }
    $statusOutput = & $cli status --ipc-path $pipe --state-path $missingFallback
    if ($LASTEXITCODE -ne 0 -or -not $statusOutput) { throw 'Windows named-pipe status query failed.' }
    $ticksOutput = & $cli ticks --limit 3 --ipc-path $pipe --state-path $missingFallback
    if ($LASTEXITCODE -ne 0 -or -not $ticksOutput) { throw 'Windows named-pipe ticks query failed.' }
    if (-not ((Get-Content -Raw -LiteralPath (Join-Path $scratch 'daemon.stdout.log')) -match '"platform":"simulated_platform"')) {
        throw 'Daemon did not report simulated platform selection.'
    }
    Write-Output "PASS: Windows simulated Platform Mode $($state.tick_count) ticks, $($state.successful_ticks) in-memory successes, $($state.failed_ticks) failures; IPC status and ticks passed; artifacts=$scratch"
}
finally {
    if ($process -and -not $process.HasExited) {
        # Terminate only the daemon process started by this script.
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
    }
    $env:AICORE_EXECUTION_MODE = $oldMode
    $env:TEMP = $oldTemp
    $env:TMP = $oldTmp
}
