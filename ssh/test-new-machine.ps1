# Tests new-machine.ps1 in offline mode, in a home directory that the test makes.
# It does not touch the real ~/.ssh. It runs the script with the same
# PowerShell that runs the test, so CI can test 5.1 and 7 apart.

$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'new-machine.ps1'
$shell = (Get-Process -Id $PID).Path
$home2 = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path (Join-Path $home2 '.ssh') | Out-Null

function Fail($why) { Write-Host "FAIL: $why"; exit 1 }

# PowerShell reads $HOME from USERPROFILE on Windows, and from HOME on other hosts.
$saved = @{ HOME = $env:HOME; USERPROFILE = $env:USERPROFILE }
$env:HOME = $home2
$env:USERPROFILE = $home2

try {
    $ssh = Join-Path $home2 '.ssh'
    $config = Join-Path $ssh 'config'
    $key = Join-Path $ssh 'id_ed25519'

    # The config has an entry before the run, with no newline at the end.
    # The script must keep it, and must not join its block to the last line.
    [IO.File]::WriteAllText($config, "Host old`n    HostName old.example")

    & $shell -NoProfile -ExecutionPolicy Bypass -File $script -NoPassphrase -Offline -Comment 'test@test' | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "the script exited with $LASTEXITCODE" }

    if (-not (Test-Path $key)) { Fail 'no private key' }
    if (-not (Test-Path "$key.pub")) { Fail 'no public key' }
    if (-not ((Get-Content "$key.pub") -match 'test@test')) { Fail 'the comment is not in the key' }

    $lines = Get-Content $config
    if (-not ($lines -match '^Host\s+old$')) { Fail 'the old entry is lost' }
    if (-not ($lines -match '^\s+HostName\s+old\.example$')) { Fail 'the last line of the old entry changed' }
    if (@($lines -match '^Host\s+github\.com$').Count -ne 1) { Fail 'no GitHub block' }

    $bytes = [IO.File]::ReadAllBytes($config)
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB) { Fail 'the config starts with a BOM' }

    # A second run keeps the key and does not add a second block.
    $before = Get-Content "$key.pub"
    & $shell -NoProfile -ExecutionPolicy Bypass -File $script -NoPassphrase -Offline | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "the second run exited with $LASTEXITCODE" }
    if ((Get-Content "$key.pub") -ne $before) { Fail 'the second run changed the key' }
    $n = @(Get-Content $config | Where-Object { $_ -match '^Host\s+github\.com$' }).Count
    if ($n -ne 1) { Fail "the second run added a block ($n blocks)" }

    # SSH must read the config without an error, and find the key path in it.
    $resolved = (& ssh -G -F $config github.com) -join "`n"
    if ($LASTEXITCODE -ne 0) { Fail 'ssh cannot read the config' }
    if ($resolved -notmatch 'identityfile .*id_ed25519') { Fail 'ssh does not read the key path from the config' }

    Write-Host 'PASS'
} finally {
    $env:HOME = $saved.HOME
    $env:USERPROFILE = $saved.USERPROFILE
    Remove-Item -Recurse -Force $home2
}
