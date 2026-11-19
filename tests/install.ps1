# Tests install.ps1 against a fake release on a local web server.
# It does not touch the real LOCALAPPDATA or the real PATH.
#
# Usage: pwsh -File tests/install.ps1 [-Odd <path of a built odd.exe>]
# On Windows, give the built odd.exe. On other hosts, a shell script stands
# in for it.

param([string]$Odd)
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$work = Join-Path ([IO.Path]::GetTempPath()) ("odd-test-" + [Guid]::NewGuid())
$release = Join-Path $work 'release'
New-Item -ItemType Directory -Path $release | Out-Null
$server = $null
$passed = $false

function Fail($why) { Write-Host "FAIL: $why"; exit 1 }

try {
    $sums = @()
    foreach ($arch in 'x86_64', 'aarch64') {
        $name = "odd-$arch-pc-windows-msvc.exe"
        $path = Join-Path $release $name
        if ($Odd) {
            Copy-Item $Odd $path
        } else {
            Set-Content -Path $path -Value "#!/bin/sh`necho `"fake odd: `$*`"" -NoNewline
            & chmod +x $path
        }
        $hash = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLower()
        $sums += "$hash  $name"
    }
    Set-Content -Path (Join-Path $release 'SHA256SUMS') -Value ($sums -join "`n")

    $python = if (Get-Command python3 -ErrorAction SilentlyContinue) { 'python3' } else { 'python' }
    # Single quotes inside: Windows PowerShell 5.1 drops the double quotes in an
    # argument that it gives to a native program.
    $port = & $python -c 'import socket; s=socket.socket(); s.bind((''127.0.0.1'', 0)); print(s.getsockname()[1])'
    $serve = @{ FilePath = $python; ArgumentList = @('-m', 'http.server', $port, '--bind', '127.0.0.1', '--directory', $release); PassThru = $true }
    if ($PSVersionTable.PSEdition -eq 'Desktop' -or $IsWindows) { $serve.WindowStyle = 'Hidden' }
    $server = Start-Process @serve
    $env:ODD_BASE_URL = "http://127.0.0.1:$port"
    $up = $false
    foreach ($i in 1..50) {
        try { Invoke-WebRequest -UseBasicParsing "$env:ODD_BASE_URL/SHA256SUMS" | Out-Null; $up = $true; break } catch { Start-Sleep -Milliseconds 100 }
    }
    if (-not $up) { Fail 'the web server did not start' }

    $env:PROCESSOR_ARCHITECTURE = if ($env:PROCESSOR_ARCHITECTURE) { $env:PROCESSOR_ARCHITECTURE } else { 'AMD64' }
    $env:ODD_NO_PATH = '1'
    $shell = (Get-Process -Id $PID).Path

    # installs_odd
    $env:LOCALAPPDATA = Join-Path $work 'local1'
    $out = (& $shell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'install.ps1') 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0) { Fail "install exited $LASTEXITCODE`: $out" }
    if (-not (Test-Path (Join-Path (Join-Path (Join-Path $env:LOCALAPPDATA 'Programs') 'odd') 'odd.exe'))) { Fail "no odd.exe: $out" }

    # arguments_run_once_and_leave_nothing
    $env:LOCALAPPDATA = Join-Path $work 'local2'
    $out = (& $shell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'install.ps1') '--version' 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0) { Fail "run once exited $LASTEXITCODE`: $out" }
    # Only Windows can run odd.exe. Elsewhere the fake cannot run, because
    # the download loses the mode that lets it run, so only the exit code
    # and the install path are checked there.
    if ($Odd -and $out -notmatch 'odd \d{4}\.\d+\.\d+ \(') { Fail "odd did not print its version: $out" }
    if (Test-Path (Join-Path (Join-Path (Join-Path $env:LOCALAPPDATA 'Programs') 'odd') 'odd.exe')) { Fail 'run once installed odd' }

    # a_bad_checksum_installs_nothing
    $bad = (Get-Content (Join-Path $release 'SHA256SUMS')) | ForEach-Object { '0' + $_.Substring(1) }
    Set-Content -Path (Join-Path $release 'SHA256SUMS') -Value ($bad -join "`n")
    $env:LOCALAPPDATA = Join-Path $work 'local3'
    $out = (& $shell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'install.ps1') 2>&1) -join "`n"
    if ($LASTEXITCODE -eq 0) { Fail 'a bad checksum exited 0' }
    if ($out -notmatch 'does not match SHA256SUMS') { Fail "no checksum message: $out" }
    if (Test-Path (Join-Path (Join-Path (Join-Path $env:LOCALAPPDATA 'Programs') 'odd') 'odd.exe')) { Fail 'a bad checksum installed odd' }

    Write-Host 'PASS'
    # The last native command can leave a non-zero LASTEXITCODE, which would
    # become the exit code of the test.
    $passed = $true
} finally {
    if ($server) { Stop-Process -Id $server.Id -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
if ($passed) { exit 0 }
