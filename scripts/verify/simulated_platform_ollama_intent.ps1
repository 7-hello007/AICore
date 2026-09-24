# End-to-end local intent over a simulated Platform Mode daemon and IPC.
# Uses an already-running Ollama service; neither installer nor host setter is invoked.
param([string]$Model = 'llama3.2:3b')
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$daemon = Join-Path $projectRoot 'target\debug\aicored.exe'
$cli = Join-Path $projectRoot 'target\debug\aicore.exe'
if (-not (Test-Path -LiteralPath $daemon) -or -not (Test-Path -LiteralPath $cli)) {
    throw 'Build aicore-daemon and aicore-cli before running this check.'
}
$scratch = Join-Path $projectRoot ('env\ollama-intent-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())
New-Item -ItemType Directory -Force -Path $scratch, (Join-Path $scratch 'plugins') | Out-Null
$pipe = '\\.\pipe\aicore-intent-' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$oldMode = $env:AICORE_EXECUTION_MODE
$oldModel = $env:AICORE_OLLAMA_MODEL
$oldBackend = $env:AICORE_INTENT_BACKEND
$oldPolicy = $env:AICORE_APP_POLICY_PATH
$oldTemp = $env:TEMP
$oldTmp = $env:TMP
$env:AICORE_EXECUTION_MODE = 'simulated-platform'
$env:AICORE_INTENT_BACKEND = 'ollama'
$env:AICORE_OLLAMA_MODEL = $Model
$pinPath = Join-Path $scratch 'application-pins.json'
$pins = @(@{
    application_id = 'aicore.intent.smoke'
    artifact_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $cli).Hash.ToLowerInvariant()
    allowed_actions = @('register', 'propose_intent', 'unregister')
})
ConvertTo-Json -InputObject $pins -Depth 4 | Set-Content -LiteralPath $pinPath -Encoding UTF8
$env:AICORE_APP_POLICY_PATH = $pinPath
$env:TEMP = $scratch
$env:TMP = $scratch
$process = $null
try {
    $arguments = @(
        '--config', (Join-Path $scratch 'config.json'),
        '--state-path', (Join-Path $scratch 'status.json'),
        '--ipc-path', $pipe,
        '--memory-path', (Join-Path $scratch 'memory.db'),
        '--plugins-dir', (Join-Path $scratch 'plugins'),
        '--interval-ms', '100'
    )
    $process = Start-Process -FilePath $daemon -ArgumentList $arguments `
        -RedirectStandardOutput (Join-Path $scratch 'daemon.stdout.log') `
        -RedirectStandardError (Join-Path $scratch 'daemon.stderr.log') `
        -WindowStyle Hidden -PassThru
    $ready = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        Start-Sleep -Milliseconds 100
        if ($process.HasExited) { break }
        if (Test-Path -LiteralPath (Join-Path $scratch 'status.json')) { $ready = $true; break }
    }
    if (-not $ready) { throw 'simulated daemon did not become ready' }
    $output = & $cli intent --text 'I am compiling a Rust project in my editor' `
        --application-id 'aicore.intent.smoke' --ipc-path $pipe 2>&1
    if ($LASTEXITCODE -ne 0) { throw "intent IPC call failed: $output" }
    $proposal = ($output | Out-String | ConvertFrom-Json)
    if ($proposal.workload_type -ne 'compilation' -or $proposal.confidence -lt 0.6) {
        throw "unusable Ollama proposal: $output"
    }
    Write-Output "PASS: pinned CLI executable and local Ollama $Model intent over simulated daemon IPC; workload=$($proposal.workload_type); confidence=$($proposal.confidence); artifacts=$scratch"
}
finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
    }
    $env:AICORE_EXECUTION_MODE = $oldMode
    $env:AICORE_OLLAMA_MODEL = $oldModel
    $env:AICORE_INTENT_BACKEND = $oldBackend
    $env:AICORE_APP_POLICY_PATH = $oldPolicy
    $env:TEMP = $oldTemp
    $env:TMP = $oldTmp
}
