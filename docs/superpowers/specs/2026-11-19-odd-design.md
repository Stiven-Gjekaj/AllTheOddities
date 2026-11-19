# Design: `odd`, the binary of AllTheOddities

Date: 2026-11-19.
Status: draft, for approval.

## 1. Intent

AllTheOddities changes from a folder of loose scripts into one command, `odd`,
in the way that AllTheTools is one command, `att`.

Each oddity is a niche job that helped once: most of them make a new computer
ready, or fix one odd problem. Each oddity is a subcommand, such as
`odd ssh`. The same subcommand works on Windows, on macOS and on Linux. It
reads the host and does the correct steps for that host.

The reason: the owner sets up new computers, and each time searches again for
the same fixes, on three systems with three different answers. One command
with one set of options keeps the fixes, and runs them again in one step.

## 2. Goals, non-goals, anti-goals

Goals:

- One binary, `odd`, for Windows, macOS and Linux, on x64 and on arm64.
- Install on a bare computer with one line, with no package manager.
- 20 oddities in the first release (section 7).
- Each oddity that changes the computer can undo its change.
- New oddities are easy to add, and they ship in the next release.

Non-goals:

- `odd` does not repeat a tool of AllTheTools. If a job is general and many
  people need it many times, it goes to `att`.
- `odd` is not a manager of dotfiles. `odd carry` copies files one time. It
  does not keep them in sync.
- `odd` does not set up many servers at a time. It works on the computer that
  runs it.
- No graphical interface.

Anti-goals (a failure even if the code works):

- A change to the computer that `odd` cannot undo, and that `odd` did not say
  before it made it.
- A secret on the disk or in the list of processes: a Tailscale auth key, a
  Wi-Fi password in a log, a private key in a state file.
- A step that needs an administrator, and that fails in the middle and leaves
  half a change.
- A test that touches the real system of the person who runs it.

## 3. Constraints

- The rules of [AGENTS.md](../../../AGENTS.md) apply: Simplified Technical
  English, one change in each commit, code and tests in the same commit,
  documentation in a different commit.
- A bare computer has no Homebrew, no Scoop, no Python and no Rust. It has:
  `curl` and `sh` on macOS and Linux (some Linux hosts have `wget` and not
  `curl`), and Windows PowerShell 5.1 with `Invoke-WebRequest` on Windows.
- Windows PowerShell 5.1 is the default shell of a new Windows computer. The
  Windows installer must run in it.
- Many oddities need an administrator. `odd` does not raise its own rights.
- The release pipeline of AllTheTools exists and works. Reuse it.

## 4. Approaches

The owner and the agent compared three ways.

| Approach | For | Against |
| :-- | :-- | :-- |
| **Rust binary (picked)** | Same language and layout as AllTheTools. The CI, the release builds, `SHA256SUMS`, the attestation and the package manifests copy over. One static binary on each host. | Slow to compile. More code than Go for "run a host command and check it". |
| Go binary | Easy cross-compile. Small binaries. | A second toolchain and a second release pipeline, made from nothing. |
| Shell and PowerShell scripts (the current state) | No download. Runs on a bare computer. | Two copies of each oddity in two languages, which drift apart. No single command. |

Python and Node drop out at once: a new Windows computer has neither.

The owner picked Rust, and the one-line installers of section 6 to solve the
download on a bare computer.

## 5. Layout

The layout copies AllTheTools:

    Cargo.toml                    workspace, exact dependency versions
    crates/alltheoddities/
      Cargo.toml                  package "alltheoddities", binary "odd"
      src/main.rs                 parses the command line, calls the oddity
      src/cli.rs                  clap definitions, the common flags
      src/oddity.rs               the Oddity trait and the list of oddities
      src/host.rs                 which host, which distribution, admin or not
      src/run.rs                  runs host commands through a Runner trait
      src/ask.rs                  questions, yes or no, hidden input
      src/state.rs                the record of changes, for undo
      src/version.rs              the version and the codename
      src/oddities/<name>.rs      one file for each oddity
    install.sh                    the installer for macOS and Linux
    install.ps1                   the installer for Windows
    docs/oddities.md              each oddity, each host, its label
    docs/releasing.md             how to cut a release
    docs/limits.md                what each oddity cannot do on each host
    CHANGELOG.md                  each release

The current `ssh/new-machine.sh`, `ssh/new-machine.ps1` and their tests go
away in the commit after `odd ssh` covers the same steps. Their test cases
move to Rust tests.

### 5.1 The Oddity trait

Each oddity is one type that gives:

- `name`: the subcommand, such as `ssh`.
- `summary`: one line for `odd list` and for `--help`.
- `hosts`: the hosts it supports. On another host, `odd` prints
  "odd <name> does not run on <host> yet" and exits with code 2.
- `label`: experimental, stable or deprecated.
- `run`: does the work.
- `undo`: reverts what `run` recorded. An oddity that changes nothing, such
  as `odd wifi-pass`, has no undo.

The list of oddities is one array in `src/oddity.rs`. To add an oddity: add
one file, and add one line to that array.

### 5.2 Common flags and exit codes

Each oddity takes these flags:

| Flag | What it does |
| :-- | :-- |
| `--yes` | Accepts each default, and asks no question. A value with no default, such as an auth key, must then come from a flag or from the environment. |
| `--dry-run` | Prints each step and each host command, and changes nothing. |
| `--undo` | Reverts the changes that the last run of this oddity recorded. |
| `--json` | Only on an oddity that reads and changes nothing. |

| Exit code | Meaning |
| :-- | :-- |
| 0 | The oddity did its work. |
| 1 | A step failed. The output says which step, and what to do. |
| 2 | The usage is not correct, or the oddity does not run on this host. |
| 3 | The oddity needs an administrator. It changed nothing. The output gives the command to run again with `sudo`, or in an administrator shell. |

### 5.3 Administrator rights

Before an oddity changes anything, it checks each step that needs an
administrator. If one step needs it and `odd` does not have it, `odd` stops
before the first change, and exits with code 3. So a run never stops halfway
for lack of rights.

### 5.4 The record of changes

Each change goes into a state file before `odd` makes it:
`~/.local/state/odd/<oddity>.json` on macOS and Linux, and
`%LOCALAPPDATA%\odd\state\<oddity>.json` on Windows. The record says what
the value was before, and what `odd` set. `--undo` reads the record and puts
back the old values, newest first. It does not touch a value that changed
again after `odd` changed it; it names that value and skips it.

The state file holds no secret.

### 5.5 Tests

No test touches the real system. The rule comes from AllTheTools.

- Each host command goes through the `Runner` trait. The tests use a fake
  runner, which records each command and gives back a prepared answer.
- Each file change takes a root directory. The tests give a temporary
  directory.
- CI runs the tests on Linux, macOS and Windows, and runs each oddity with
  `--dry-run` on each host it supports.

## 6. Install and update

### 6.1 The installers

macOS and Linux:

    curl -fsSL https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.sh | sh

Windows, in Windows PowerShell 5.1 or PowerShell 7:

    irm https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.ps1 | iex

Each installer:

1. Finds the host and the processor: x64 or arm64.
2. Downloads the binary of that target, and `SHA256SUMS`, from the latest
   release. The URL is
   `https://github.com/Stiven-Gjekaj/AllTheOddities/releases/latest/download/<file>`.
   `ODD_VERSION=v2026.12.0` selects a given release.
3. Compares the SHA-256 of the binary with `SHA256SUMS`, with `sha256sum`,
   `shasum -a 256` or `Get-FileHash`. A mismatch stops the install, and
   the installer installs nothing.
4. Puts `odd` in `~/.local/bin`, or in `%LOCALAPPDATA%\Programs\odd`.
5. Runs `odd path add <that directory>`, so the next shell finds `odd`.

The shell installer uses `wget` when the host has no `curl`. Each installer
stays short, so that a careful person can read it before they run it.

### 6.2 Run one time

    curl -fsSL .../install.sh | sh -s -- ssh

    & ([scriptblock]::Create((irm .../install.ps1))) ssh

With arguments, the installer downloads and checks the binary in a temporary
directory, runs `odd` with those arguments, and deletes the directory. It
installs nothing. This suits a computer that you only set up.

### 6.3 Update

`odd update` reads the latest release, and stops if it is the version that
runs. Otherwise it downloads and checks the new binary as the installer does,
and replaces itself. On Windows, nothing can overwrite a running binary, but a rename
works. So `odd` renames itself to `odd.old.exe`, writes the new
binary, and deletes `odd.old.exe` on the next start.

### 6.4 Package managers

Homebrew, Scoop and winget packages follow, with the manifest scripts of
AllTheTools. They are for computers that have those tools already. The
installers stay the main way.

## 7. The 20 oddities of the first release

Each one ships as **experimental** in the first release. An oddity becomes
stable when CI passes on each host that it supports and the owner ran it on a
real computer of each host.

| # | Command | What it does | Hosts | Admin | Undo |
| :-- | :-- | :-- | :-- | :-- | :-- |
| 1 | `odd ssh` | Makes SSH ready, out and in, and joins Tailscale (7.1) | all | for "in" and Tailscale | yes |
| 2 | `odd git` | Sets the commit name and email for each folder with `includeIf`, and signs commits with the SSH key | all | no | yes |
| 3 | `odd carry` | Sends chosen files to another computer with Taildrop (`tailscale file cp`), and receives them on the other side | all | no | no |
| 4 | `odd expose <port>` | Shares a local port on the tailnet with `tailscale serve`, or in public with `--public` (Funnel), until Ctrl+C | all | no | stops on exit |
| 5 | `odd wifi-pass` | Shows the password of the current or a named Wi-Fi network, and a QR code that a phone can scan | all | macOS asks for the keychain; Linux may need it | no |
| 6 | `odd hosts` | Adds and removes entries in the hosts file, in one block that `odd` manages | all | yes | yes |
| 7 | `odd path` | Adds a directory to the PATH of the user for good, and lists entries that repeat or do not exist | all | no | yes |
| 8 | `odd env` | Sets an environment variable of the user for good | all | no | yes |
| 9 | `odd tweaks` | Applies developer settings. Windows: show file extensions and hidden files, long paths, Developer Mode. macOS: show hidden files, fast key repeat. Linux: a higher inotify watch limit | all | some steps | yes |
| 10 | `odd dualboot-clock` | Makes Windows keep the hardware clock in UTC, so the clock does not jump between Windows and Linux | Windows, Linux | yes | yes |
| 11 | `odd font <file or name>` | Installs a font for the user only. A name such as `JetBrainsMono` downloads that Nerd Font from its GitHub release | all | no | yes |
| 12 | `odd hostname <name>` | Sets the name of the computer in each place that the host keeps it (macOS has three) | all | yes | yes |
| 13 | `odd share <path>` | Serves a file or a folder over HTTP on the local network, with a one-time token in the URL, and prints the URL and a QR code, until Ctrl+C | all | no | stops on exit |
| 14 | `odd devcert` | Makes a local certificate authority and a certificate for `localhost`, and makes the host trust the authority | all | Linux yes | yes |
| 15 | `odd firewall open/close <port>` | Opens or closes a port in the firewall of the host (`netsh advfirewall`, `ufw` or `firewalld`) | Windows, Linux | yes | yes |
| 16 | `odd autostart add/remove/list` | Starts a command at login: a launchd agent, a systemd user unit, or a Task Scheduler task | all | no | yes |
| 17 | `odd cache-purge` | Lists the caches of developer tools and their size, and removes the ones you select | all | no | no |
| 18 | `odd dns` | Sets the DNS servers of the active network, with presets such as `cloudflare` or `quad9`, and `--reset` for the DHCP servers | all | yes | yes |
| 19 | `odd apps` | Installs apps from one list, with winget, Homebrew, apt, dnf or pacman | all | Linux yes | no |
| 20 | `odd remote` | Turns on remote desktop: Screen Sharing on macOS, Remote Desktop on Windows, GNOME Remote Desktop on Linux | all, with limits | yes | yes |

`odd list`, `odd update` and `odd --version` are commands of `odd` itself.
They do not count as oddities.

### 7.1 `odd ssh` in detail

`odd ssh` finds the host and asks three questions, in this order. `odd` skips each
part that is done already. The flags `--out`, `--in` and `--tailscale`
select the parts with no question.

**Out: connect from this computer.** The steps of the current scripts:
make `~/.ssh`; make an ed25519 key if none exists; add a `github.com` block to
`~/.ssh/config`; give the key to the agent (the keychain on macOS, the
`ssh-agent` service on Windows); on Windows, set `core.sshCommand` of Git to
the ssh of Windows if it is not set; send the public key to GitHub with `gh`
if `gh` is signed in, else print it with the link; test with
`ssh -T git@github.com`.

**In: let other computers connect to this one.** Needs an administrator.

| Host | Steps |
| :-- | :-- |
| macOS | Turn on Remote Login with `systemsetup -setremotelogin on`. |
| Linux | Install `openssh-server` with the package manager of the distribution, then `systemctl enable --now` the service (`ssh` or `sshd`, by distribution). |
| Windows | `Add-WindowsCapability` for `OpenSSH.Server`, set the `sshd` service to start automatically, start it, and make sure the firewall rule for port 22 exists. |

Then `odd` asks for a GitHub user name, reads
`https://github.com/<user>.keys`, and adds each key that is not there already
to the list of authorized keys. On Windows, a user in the Administrators
group needs `C:\ProgramData\ssh\administrators_authorized_keys` with the
correct access list, and not `~/.ssh/authorized_keys`. `odd` writes the file
that applies.

**Tailscale: reach this computer from anywhere.** If the host has no
Tailscale, `odd` asks, then installs it with the official way for the host:
the install script of Tailscale on Linux, the package from
`pkgs.tailscale.com` on macOS, and `winget` or the MSI on Windows. Then it
asks for an auth key with hidden input, or reads `TS_AUTHKEY`. The owner
picked an **ephemeral** auth key: the computer leaves the tailnet soon after
it goes offline, and no cleanup is needed. `odd` tells the user to make the
key ephemeral and single-use, and links the page of the admin console where
they make it.

The key never goes on the command line, where the list of processes shows
it. `odd` writes it to a temporary file that only the user can read, runs
`tailscale up --auth-key=file:<path>`, and deletes the file at once.

At the end, `odd` prints the tailnet name and the address of the computer,
and the command to connect: `ssh <user>@<name>`.

`--tailscale-ssh` adds `--ssh` to `tailscale up`, so Tailscale SSH serves the
connections in place of sshd. It is on macOS and Linux only, because Tailscale
SSH has no server on Windows.

`odd ssh --undo` turns off what `odd` turned on: the server, the firewall
rule, the authorized keys that it added, and `tailscale logout`. It keeps the
key pair, because a key that is deleted cannot come back.

### 7.2 Calls the agent made for the other oddities

The builder check found these gaps. The agent settled them, and the owner can
change them.

- **`odd git`** takes profiles: `odd git add <folder> --name <name> --email <email>`
  writes a file `~/.gitconfig-<folder name>` and an `includeIf "gitdir:<folder>/"`
  line in `~/.gitconfig`. `--sign` sets `gpg.format ssh`, `user.signingkey`
  to the public key of `odd ssh`, `commit.gpgsign true`, and adds the key to
  `~/.ssh/allowed_signers`.
- **`odd path` and `odd env`** write to the user scope only. Windows: the
  `HKCU\Environment` key, then a `WM_SETTINGCHANGE` broadcast, so new windows
  see the change. macOS and Linux: one managed block in `~/.profile`, in
  `~/.zprofile` if the shell of the user is zsh, and `fish_add_path` or
  `set -Ux` if it is fish. Each block starts and ends with a marker line, so
  `--undo` removes only that block.
- **`odd apps`** reads `~/.config/odd/apps.toml` on macOS and Linux and
  `%APPDATA%\odd\apps.toml` on Windows. Each entry has one name and the
  package id for each manager. `odd` ships a catalog of common apps, so an
  entry can be one name only. On a Mac with no Homebrew, `odd` asks, then
  installs Homebrew with its official script.
- **`odd carry`** has two sides: `odd carry send <computer> [files]` and
  `odd carry receive`. Both computers must be on the same tailnet, which
  `odd ssh --tailscale` gives. With no files, it offers `~/.ssh/config`,
  `~/.gitconfig`, `~/.gitconfig-*`, and the shell profile. It never offers a
  private key unless the user names it.
- **`odd dualboot-clock`** changes Windows by default: it sets
  `RealTimeIsUniversal` to 1 in
  `HKLM\SYSTEM\CurrentControlSet\Control\TimeZoneInformation`. On Linux,
  `--linux-local` runs `timedatectl set-local-rtc 1` in its place. One side
  is enough, and `odd` says so.

## 8. Releases

### 8.1 The version

The version is the date of the release and a count: `YYYY.MM.N`. The first
release of a month has N = 0. Each next release in the same month adds one,
for a fix or for a new oddity. The tag is `v2026.12.0`. The version is also
valid semver, so Cargo takes it as it is.

### 8.2 The codename

Each month that has a release gets a codename. The release of a later month
takes the next codename on the list, so the letters go in order and no
letter is lost when a month has no release. A `.N` release keeps the
codename of its month.

The theme is a cabinet of curiosities, the room of odd objects that collectors
kept from the 16th century:

Astrolabe, Barometer, Chronometer, Diorama, Ephemeris, Fossil, Gyroscope,
Hourglass, Inclinometer, Jacquard, Kaleidoscope, Lodestone, Microscope,
Narwhal, Orrery, Planisphere, Quadrant, Reliquary, Sextant, Telescope,
Unicorn, Vellum, Wunderkammer, Xylograph, Yardstick, Zoetrope.

A search on 2026-11-19 found no software project that uses this theme for its
releases.

The release title is the version and the codename: `2026.12 "Astrolabe"`.
`odd --version` prints `odd 2026.12.0 (Astrolabe)`.

### 8.3 The labels

| Label | What it promises |
| :-- | :-- |
| experimental | Nothing. Flags and output can change in any release. |
| stable | Its flags and exit codes stay the same. A change that breaks them needs a new oddity name, or a deprecation first. |
| deprecated | It still runs, and prints a warning. It goes away no earlier than the release after the one that marked it. |

`odd list` shows each oddity with its label. Each release note has four
parts: New, Now stable, Fixed, Removed. Each entry names the oddity.

### 8.4 The pipeline

Copy `release.yml` and `ci.yml` of AllTheTools, with these changes: the
package is `alltheoddities`, the binary is `odd`, the files are named
`odd-<target>` (`odd-<target>.exe` on Windows), and the installers are not
part of a release (they come from the `main` branch, so a fix to an installer
reaches each user at once). The six targets stay the same as in AllTheTools.

## 9. Left to the builder

These are small choices that do not change what gets built:

- The crate for each job, for example `qrcode` for the QR codes,
  `tiny_http` for `odd share`, `rcgen` for `odd devcert`, `rpassword` for
  hidden input. Pin each version, as AllTheTools does.
- The text of each question and each message, in Simplified Technical
  English.
- The list of caches in `odd cache-purge`, and the apps in the list of
  `odd apps`, as long as each is easy to extend.
- The order in which the 20 oddities get built. `odd ssh`, `odd path` (the
  installer needs it) and `odd list` come first.

## 10. To check before the build

The builder confirms these facts on a real host or in the documentation of the
host before relying on them:

- `tailscale up --auth-key=file:<path>` reads the key from a file on each
  host.
- `systemsetup -setremotelogin on` works on the current macOS from a
  terminal that has no Full Disk Access. If it does not, `odd` gives the
  steps in System Settings instead.
- Windows Home has no Remote Desktop server. `odd remote` says so on that
  edition, and exits with code 2.
- A file that `curl` or `Invoke-WebRequest` downloads gets no download mark,
  so macOS and SmartScreen do not block `odd`.
