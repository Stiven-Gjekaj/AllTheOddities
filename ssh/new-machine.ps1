<#
.SYNOPSIS
Makes a new Windows computer ready to use SSH with GitHub.

.DESCRIPTION
It runs in Windows PowerShell 5.1 and in PowerShell 7. You can run it again:
each step that is done already is skipped.

The steps:
  1. Make ~/.ssh.
  2. Make an ed25519 key, if the key does not exist.
  3. Add a GitHub block to ~/.ssh/config.
  4. Start the ssh-agent service, and give the key to it.
  5. Tell Git to use the OpenSSH of Windows, so that Git uses the agent.
  6. Send the public key to GitHub with gh, if gh is signed in.
  7. Test the connection to GitHub.

Step 4 needs an administrator the first time, because Windows turns the
ssh-agent service off by default. Step 5 changes core.sshCommand in your
global Git config, but only if you did not set it.

Exit codes: 0 when GitHub accepts the key, 1 when it does not, 2 when an
option is not correct.

.PARAMETER Comment
The comment of the key. The default is user@host.

.PARAMETER KeyFile
The path of the key. The default is ~/.ssh/id_ed25519.

.PARAMETER NoPassphrase
Make the key with no passphrase.

.PARAMETER Offline
Do not use the agent, Git, gh, or the network.

.EXAMPLE
powershell -ExecutionPolicy Bypass -File ssh\new-machine.ps1
#>

[CmdletBinding()]
param(
    [string]$Comment = "$([Environment]::UserName)@$([Environment]::MachineName)",
    [string]$KeyFile = (Join-Path (Join-Path $HOME '.ssh') 'id_ed25519'),
    [switch]$NoPassphrase,
    [switch]$Offline
)

$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 does not have $IsWindows. Its edition is Desktop.
$onWindows = ($PSVersionTable.PSEdition -eq 'Desktop') -or $IsWindows
if (-not $onWindows -and -not $Offline) {
    Write-Host 'This script is for Windows. On macOS and Linux, use ssh/new-machine.sh.'
    exit 2
}

$sshDir = Join-Path $HOME '.ssh'
$config = Join-Path $sshDir 'config'
$marker = '# AllTheOddities: github'
$noBom = New-Object System.Text.UTF8Encoding($false)

function Test-Command($name) {
    [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

if (-not (Test-Command 'ssh-keygen')) {
    Write-Host 'ssh-keygen is not found. Add the OpenSSH Client in Settings > System > Optional features.'
    exit 1
}

# 1. The directory.
New-Item -ItemType Directory -Force -Path $sshDir | Out-Null

# 2. The key.
if (Test-Path $KeyFile) {
    Write-Host "Key exists: $KeyFile. It is kept."
} else {
    if ($NoPassphrase) {
        # Windows PowerShell before 7.3 drops an empty argument, so it must
        # send the two quote characters.
        $empty = if ($PSVersionTable.PSVersion -lt [version]'7.3') { '""' } else { '' }
        & ssh-keygen -q -t ed25519 -C $Comment -f $KeyFile -N $empty
    } else {
        & ssh-keygen -q -t ed25519 -C $Comment -f $KeyFile
    }
    if ($LASTEXITCODE -ne 0) { Write-Host 'ssh-keygen failed.'; exit 1 }
    Write-Host "Key made: $KeyFile"
}

# 3. The config block. The marker line shows that the block is there.
# OpenSSH reads a forward slash in a Windows path, and no escape is needed.
$text = if (Test-Path $config) { [IO.File]::ReadAllText($config) } else { '' }
if ($text.Contains($marker)) {
    Write-Host "The GitHub block is in $config already."
} else {
    $nl = [Environment]::NewLine
    $block = ''
    if ($text.Length -gt 0) {
        if (-not $text.EndsWith("`n")) { $block += $nl }
        $block += $nl
    }
    $block += (@(
        $marker
        'Host github.com'
        '    HostName github.com'
        '    User git'
        "    IdentityFile $($KeyFile -replace '\\', '/')"
        '    IdentitiesOnly yes'
        '    AddKeysToAgent yes'
    ) -join $nl) + $nl
    [IO.File]::AppendAllText($config, $block, $noBom)
    Write-Host "The GitHub block is added to $config."
}

if ($Offline) {
    Write-Host 'Offline: the agent, Git, gh and the test are skipped.'
    exit 0
}

# 4. The agent.
$agent = Get-Service ssh-agent -ErrorAction SilentlyContinue
if (-not $agent) {
    Write-Host 'The ssh-agent service is not found. The key is not given to an agent.'
} else {
    if ($agent.Status -ne 'Running') {
        $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
            ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
        if ($admin) {
            Set-Service ssh-agent -StartupType Automatic
            Start-Service ssh-agent
            Write-Host 'The ssh-agent service runs, and starts with Windows.'
        } elseif ($agent.StartType -ne 'Disabled') {
            Start-Service ssh-agent
        } else {
            Write-Host 'The ssh-agent service is off. Run this script one time as an administrator to turn it on.'
        }
    }
    if ((Get-Service ssh-agent).Status -eq 'Running') {
        & ssh-add $KeyFile
        if ($LASTEXITCODE -ne 0) { Write-Host 'The agent did not take the key.' }
    }
}

# 5. Git. Git for Windows has its own ssh, which does not use the agent of
# Windows. So Git asks for the passphrase each time. The ssh of Windows fixes it.
if (Test-Command 'git') {
    $current = & git config --global core.sshCommand
    $winSsh = (Get-Command ssh).Source -replace '\\', '/'
    if ($current) {
        Write-Host "Git uses this ssh already: $current. It is kept."
    } else {
        & git config --global core.sshCommand $winSsh
        Write-Host "Git uses $winSsh now."
    }
}

# 6. GitHub. A key that GitHub has already gives an error, which is not a fault.
$signedIn = $false
if (Test-Command 'gh') {
    & gh auth status *> $null
    $signedIn = ($LASTEXITCODE -eq 0)
}
if ($signedIn) {
    $title = [Environment]::MachineName
    & gh ssh-key add "$KeyFile.pub" --title $title 2> $null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "The key is on GitHub with the title `"$title`"."
    } else {
        Write-Host 'gh did not add the key. GitHub can have it already.'
    }
} else {
    Write-Host 'gh is not signed in. Add this key at https://github.com/settings/ssh/new:'
    Get-Content "$KeyFile.pub"
}

# 7. The test. GitHub ends the session with exit code 1 when the key is good.
$out = (& ssh -o StrictHostKeyChecking=accept-new -T git@github.com 2>&1) -join "`n"
if ($out -match 'successfully authenticated') {
    Write-Host 'GitHub accepts the key.'
    exit 0
}
Write-Host 'GitHub does not accept the key yet:'
Write-Host $out
exit 1
