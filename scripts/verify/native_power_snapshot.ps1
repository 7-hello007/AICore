# Native power API certification using only a disposable clone of the active scheme.
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$probe=Join-Path $root 'target\debug\examples\native_power_snapshot_probe.exe'
$originalText=(& powercfg /getactivescheme | Out-String)
$guidPattern='[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}'
$original=[regex]::Match($originalText,$guidPattern).Value.ToLowerInvariant()
if (-not $original) {throw 'Cannot capture original power GUID'}
$plansBefore=([regex]::Matches((& powercfg /list | Out-String),$guidPattern) | ForEach-Object {$_.Value.ToLowerInvariant()} | Sort-Object) -join ','
$clone=[guid]::NewGuid().ToString()
$priorEnv=$env:AICORE_POWER_PROBE_SCHEME
try {
    Write-Output "NATIVE BEFORE device=power scheme list original=$plansBefore proposed=create owned clone $clone; active remains $original; rollback=powercfg /setactive $original; powercfg /delete $clone"
    & powercfg /duplicatescheme $original $clone
    if ($LASTEXITCODE -ne 0) {throw 'Owned scheme clone failed'}
    $env:AICORE_POWER_PROBE_SCHEME=$clone
    & $probe
    if ($LASTEXITCODE -ne 0) {throw 'Native power API probe failed'}
}
finally {
    $current=[regex]::Match((& powercfg /getactivescheme | Out-String),$guidPattern).Value.ToLowerInvariant()
    Write-Output "NATIVE RESTORE device=power original_now=$current proposed=$original rollback=powercfg /setactive $original"
    & powercfg /setactive $original
    $restoreCode=$LASTEXITCODE
    $restored=[regex]::Match((& powercfg /getactivescheme | Out-String),$guidPattern).Value.ToLowerInvariant()
    $plansCurrent=[regex]::Matches((& powercfg /list | Out-String),$guidPattern) | ForEach-Object {$_.Value.ToLowerInvariant()}
    if ($plansCurrent -contains $clone -and $restored -eq $original) {
        Write-Output "NATIVE CLEANUP device=power owned clone=$clone original=present proposed=delete only test clone; rollback=powercfg /duplicatescheme $original $clone"
        & powercfg /delete $clone
        if ($LASTEXITCODE -ne 0) {throw "RESTORATION FAIL deleting owned clone $clone"}
    }
    $plansAfter=([regex]::Matches((& powercfg /list | Out-String),$guidPattern) | ForEach-Object {$_.Value.ToLowerInvariant()} | Sort-Object) -join ','
    $env:AICORE_POWER_PROBE_SCHEME=$priorEnv
    if ($restoreCode -ne 0 -or $restored -ne $original -or $plansBefore -ne $plansAfter) {throw "RESTORATION FAIL active=$restored plans=$plansAfter expected=$plansBefore"}
    Write-Output "RESTORATION PASS active=$restored scheme list identical=$plansAfter"
}
