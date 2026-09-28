# Run from a DIFFERENT interactive Windows login while a test daemon owns the pipe.
# This probe opens only a named pipe; it does not install or change host state.
param(
    [Parameter(Mandatory=$true)][string]$PipeName,
    [Parameter(Mandatory=$true)][string]$OwnerSid
)
$ErrorActionPreference='Stop'
$callerSid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
if ($callerSid -eq $OwnerSid) {
    throw "NOT VERIFIED: client SID equals pipe owner SID ($callerSid)"
}
if (-not $PipeName.StartsWith('\\.\pipe\')) {
    throw 'PipeName must be a local Windows named pipe'
}
$name=$PipeName.Substring('\\.\pipe\'.Length)
if (-not $name.StartsWith('aicore-acl-')) {
    throw 'Only aicore-acl- test pipes are allowed'
}
Write-Output "BEFORE target=pipe $PipeName original=no client connection, callerSID=$callerSid ownerSID=$OwnerSid proposed=read-only connection attempt rollback=dispose client; no host settings"
$stream=[System.IO.Pipes.NamedPipeClientStream]::new('.', $name, [System.IO.Pipes.PipeDirection]::InOut)
try {
    try {
        $stream.Connect(3000)
    }
    catch [System.UnauthorizedAccessException] {
        $code=$_.Exception.HResult -band 0xFFFF
        if ($code -ne 5) { throw "FAIL: unexpected unauthorized HRESULT $code" }
        Write-Output "PASS native cross-user ACL denied with ACCESS_DENIED=5 callerSID=$callerSid ownerSID=$OwnerSid"
        return
    }
    catch [System.TimeoutException] {
        throw "NOT VERIFIED: test server absent/unreachable for $PipeName"
    }
    throw "FAIL: second login user unexpectedly connected to $PipeName"
}
finally {
    $stream.Dispose()
    Write-Output "RESTORATION PASS caller=$callerSid pipe client disposed; no persistent changes"
}
