# Installs odd, the command of AllTheOddities, on Windows.
# It runs in Windows PowerShell 5.1 and in PowerShell 7.
#
#   irm https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.ps1 | iex
#
# With arguments, it runs odd one time and installs nothing:
#
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.ps1))) ssh
#
# $env:ODD_VERSION = 'v2026.12.0' selects a release. The default is the
# latest one. The script checks the binary against SHA256SUMS before it runs it.

param([Parameter(ValueFromRemainingArguments = $true)][string[]]$OddArgs)

# Downloads odd into $dir and checks it. Gives the path, or $null when the
# checksum does not match. It writes nothing else to the output, so the
# caller gets the path only.
function Get-Odd([string]$dir) {
    $ProgressPreference = 'SilentlyContinue'
    # Windows PowerShell 5.1 does not offer TLS 1.2 by default.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'https://github.com/Stiven-Gjekaj/AllTheOddities'
    if ($env:ODD_BASE_URL) { $base = $env:ODD_BASE_URL } # for the tests only
    elseif ($env:ODD_VERSION) { $base = "$repo/releases/download/$env:ODD_VERSION" }
    else { $base = "$repo/releases/latest/download" }

    $cpu = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    $arch = if ($cpu -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
    $file = "odd-$arch-pc-windows-msvc.exe"

    $exe = Join-Path $dir 'odd.exe'
    $sums = Join-Path $dir 'SHA256SUMS'
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$file" -OutFile $exe
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums
    $want = Get-Content $sums | ForEach-Object {
        $p = $_ -split '\s+'
        if ($p.Count -ge 2 -and ($p[1] -eq $file -or $p[1] -eq "*$file")) { $p[0].ToLower() }
    } | Select-Object -First 1
    $got = (Get-FileHash -Algorithm SHA256 $exe).Hash.ToLower()
    if (-not $want -or $want -ne $got) {
        Write-Host "The checksum of $file does not match SHA256SUMS. Nothing is installed."
        return $null
    }
    return $exe
}

$ErrorActionPreference = 'Stop'
$code = 0
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('odd-' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $exe = Get-Odd $tmp
    if (-not $exe) {
        $code = 1
    } elseif ($OddArgs.Count -gt 0) {
        & $exe @OddArgs
        $code = $LASTEXITCODE
    } else {
        $dir = Join-Path (Join-Path $env:LOCALAPPDATA 'Programs') 'odd'
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $target = Join-Path $dir 'odd.exe'
        Move-Item -Force $exe $target
        Write-Host "odd is in $target."
        if (-not $env:ODD_NO_PATH) {
            & $target path add $dir --yes
            if ($LASTEXITCODE -ne 0) { Write-Host "Add $dir to PATH to run odd from any directory." }
        }
        & $target --version
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

# With irm | iex, exit would close the window of the person. So exit only
# when the script runs as a file.
if ($PSCommandPath) { exit $code }
