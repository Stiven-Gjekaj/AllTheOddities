<div align="center">
  <a href="../README.md"><b>AllTheOddities</b></a>
</div>

# The oddities

This document says what each oddity changes on each host, and what `--undo`
does. Each oddity is **experimental** until the first release.

Each change goes into a record before `odd` makes it:
`~/.local/state/odd/<oddity>.json` on macOS and Linux, and
`%LOCALAPPDATA%\odd\state\<oddity>.json` on Windows. `--undo` reads the record
and reverts the changes, newest first. It keeps a file that changed again
after `odd` wrote it, and names that file. The record holds no secret.

## odd path

Adds a directory to the PATH of the user for good.

| Host | What `odd path add <dir>` changes | What undo does |
| :-- | :-- | :-- |
| macOS, Linux | A block of lines `export PATH="<dir>:$PATH"` in `~/.profile`, in `~/.zprofile` when the shell is zsh, and in `~/.bash_profile` when the shell is bash and that file exists | Removes the block. Deletes a file that `odd` made. |
| macOS, Linux with fish | Runs `fish_add_path -U <dir>` | Takes the directory out of `fish_user_paths` |
| Windows | Adds the directory to the user `Path` in the registry, keeps each `%VARIABLE%` in it, and tells open programs that the environment changed | Removes that entry, and only that entry |

A directory that PATH has already, with or without a trailing slash, or in
other letter case on Windows, changes nothing.

`odd path list` prints each entry of the PATH, and marks an entry that does
not exist (`missing`) or that comes again (`repeat`).

## odd ssh

Makes SSH ready, in three parts. With no part flag, `odd ssh` asks about each
part. `--out`, `--in` and `--tailscale` select the parts with no question.

### Out: connect from this computer to GitHub

| Step | What it changes | When it skips the step |
| :-- | :-- | :-- |
| 1 | Makes `~/.ssh`, and sets it to `700` on macOS and Linux | Never. The step is safe to do again. |
| 2 | Makes an ed25519 key with `ssh-keygen`. `ssh-keygen` asks for the passphrase itself, so `odd` never sees it. `--no-passphrase` makes a key with none. | The key exists. `odd` keeps it. |
| 3 | Adds a block for `github.com` to `~/.ssh/config`, with `UseKeychain yes` on macOS | The block is there. |
| 4 | Gives the key to the agent: the keychain on macOS, the agent of `SSH_AUTH_SOCK` on Linux, the `ssh-agent` service on Windows. As an administrator on Windows, it sets the service to start with Windows. | No agent runs on Linux, or the service is off on Windows and `odd` is not an administrator. `odd` says how to fix it. |
| 5 | Windows only: sets `core.sshCommand` of Git to the ssh of Windows, because the ssh of Git for Windows does not use the agent of Windows | Git is not installed, or you set `core.sshCommand` already. |
| 6 | Sends the public key to GitHub with `gh ssh-key add` | `gh` has no signed-in account. `odd` prints the key and the link. |
| 7 | Tests the key with `ssh -T git@github.com` | `--dry-run` |

Under `sudo`, the out part stops before a change, because the key would
belong to root.

Undo removes the config block and the Git setting. It never deletes a key: a
deleted key cannot come back.

### In: let other computers connect to this one

This part needs an administrator.

| Host | What it changes |
| :-- | :-- |
| macOS | Turns on Remote Login with `systemsetup`. macOS can ask for Full Disk Access for this. Then `odd` gives the steps in System Settings. |
| Linux | Installs the SSH server with the package manager of the distribution, if neither the `ssh` nor the `sshd` unit exists, and starts it with `systemctl enable --now` |
| Windows | Adds the OpenSSH server feature if Windows does not have it, starts the `sshd` service and sets it to start with Windows, and adds the firewall rule `OpenSSH-Server-In-TCP` for port 22 |

Then `odd` asks for a GitHub user, or takes `--github-user`, reads
`https://github.com/<user>.keys`, and adds each key that is not there. On
Windows, the SSH server reads the keys of a member of Administrators from
`C:\ProgramData\ssh\administrators_authorized_keys`. `odd` writes that file,
and gives it to Administrators and SYSTEM only, by their SIDs, so it works on
Windows in each language.

**Keys only.** Then `odd` turns off password login:

1. It does so only when at least one key can log in. Otherwise passwords stay
   on, and `odd` says why.
2. It writes `PasswordAuthentication no` and
   `ChallengeResponseAuthentication no` to
   `/etc/ssh/sshd_config.d/50-odd.conf` when the main config includes that
   directory. Otherwise it adds them in a block before the first `Match`
   line of `sshd_config`, because a line after `Match` applies only to that
   match.
3. It checks the config with `sshd -t`. If the check fails, it puts back the
   old config, and restarts nothing.
4. It restarts the server, and asks you to test a key login from a second
   window before you close the first one.

`--keep-passwords` skips the keys-only step.

Undo turns off the server and the firewall rule that `odd` turned on, removes
the keys that `odd` added, and turns password login back on. The SSH server
program stays on the disk.

### Tailscale: reach this computer from anywhere

This part needs an administrator.

1. If the host has no Tailscale, `odd` asks, then installs it: the install
   script of Tailscale on Linux, the package of `pkgs.tailscale.com` on macOS,
   and `winget` on Windows. On macOS, allow the VPN configuration of Tailscale
   when macOS asks.
   On a Linux that the install script does not know, such as the SystemRescue
   live system, and when the script fails, `odd` downloads the static
   binaries of `pkgs.tailscale.com`. It unpacks them into its state
   directory, `~/.local/state/odd/tailscale`.
2. On Linux, it makes sure that `tailscaled` runs. It starts the systemd unit
   `tailscaled` when the host has one. Otherwise it starts `tailscaled` in
   the background with `--tun=userspace-networking --state=mem:`. It waits
   until `tailscaled` answers.
3. It reads the auth key from `TS_AUTHKEY`, or asks for it with hidden input.
   Make the key **ephemeral** and single-use at
   <https://login.tailscale.com/admin/settings/keys>. An ephemeral computer
   leaves the tailnet soon after it goes offline.
4. It writes the key to a file that only you can read, runs
   `tailscale up --auth-key=file:<path>`, and deletes the file at once. The
   key never goes on a command line, where the list of processes shows it.
5. It prints the tailnet name and address of the computer, and the command
   to connect: `ssh <user>@<name>`.

`--tailscale-ssh` lets Tailscale SSH serve the connections, on macOS and
Linux. Tailscale SSH has no server on Windows.

Undo runs `tailscale logout`.

## odd git

`odd git add <folder> --name <name> --email <email> [--sign]`

| What it changes | What undo does |
| :-- | :-- |
| Writes `~/.gitconfig-<folder>` with the name and the email | Deletes the file, or puts back the text before |
| Adds `[includeIf "gitdir/i:<folder>/"]` with the path of that file to a block of `~/.gitconfig` (`gitdir:` on Linux, which has case-sensitive paths) | Removes the block |
| With `--sign`: signs commits and tags with `~/.ssh/id_ed25519.pub`, and adds the key to `~/.ssh/allowed_signers` | Puts back `allowed_signers` |

The folder must exist. `odd` resolves its links, because git compares the
resolved path. `odd git list` prints each folder with its name and email.

## odd env

`odd env set <NAME> <value>`

| Host | What it changes |
| :-- | :-- |
| macOS, Linux | A line `export NAME='value'` in a block of the same profiles as `odd path` |
| macOS, Linux with fish | `set -Ux NAME value` |
| Windows | The value `NAME` of `HKCU\Environment`, as `REG_SZ`, so a `%` stays as it is |

Undo removes the line, or puts back the value before on Windows, or deletes a
value that did not exist.

## odd tweaks

`odd tweaks` lists the tweaks of the host, on or off. `odd tweaks apply`
turns on the ones that are off. `--only <name>` selects tweaks, and
`--user-only` skips the ones that need an administrator.

| Host | Tweak | Administrator | What it sets |
| :-- | :-- | :-- | :-- |
| Windows | file-extensions | no | `HideFileExt` = 0 |
| Windows | hidden-files | no | `Hidden` = 1 |
| Windows | long-paths | yes | `LongPathsEnabled` = 1 |
| Windows | developer-mode | yes | `AllowDevelopmentWithoutDevLicense` = 1 |
| macOS | hidden-files | no | `com.apple.finder AppleShowAllFiles` = true, then restarts Finder |
| macOS | key-repeat | no | `KeyRepeat` = 2, `InitialKeyRepeat` = 15, from the next login |
| Linux | inotify-watches | yes | `fs.inotify.max_user_watches = 524288` in `/etc/sysctl.d/90-odd.conf` |

Undo puts back each value before, or deletes a value that did not exist.

## odd hostname

`odd hostname <name>` needs an administrator. `odd hostname` prints the names.

| Host | What it changes |
| :-- | :-- |
| macOS | `ComputerName`, `LocalHostName` and `HostName` with `scutil` |
| Linux | `hostnamectl set-hostname`, and the old name on the `127.0.1.1` line of `/etc/hosts` |
| Windows | `Rename-Computer`. The new name comes after the next restart. |

The name has 1 to 63 letters, digits and hyphens, and at most 15 characters
on Windows. Undo sets the old names back.

## odd hosts

`odd hosts add <ip> <name>...`, `odd hosts remove <name>`, `odd hosts list`.
`add` and `remove` need an administrator. Each entry goes in a block of
`/etc/hosts`, or of `C:\Windows\System32\drivers\etc\hosts`. A name that
the block has already moves to the new address. Undo removes the block, and
keeps each line that the person wrote.

## odd dns

`odd dns set <cloudflare | google | quad9 | address...>`, `odd dns reset`,
`odd dns show`. `set` and `reset` need an administrator.

| Host | The network in use | What it changes |
| :-- | :-- | :-- |
| macOS | The service of the interface of the default route | `networksetup -setdnsservers` |
| Linux | The NetworkManager connection of the device of the default route | `ipv4.dns` and `ipv4.ignore-auto-dns` with `nmcli`, then `nmcli connection up` |
| Windows | The interface of the default route with the lowest metric | `Set-DnsClientServerAddress` |

Undo puts back the servers before, or the servers of DHCP when there were
none.

## odd wifi-pass

`odd wifi-pass [<network>] [--qr]` changes nothing. With no network, it reads
the current one.

| Host | The current network | The password |
| :-- | :-- | :-- |
| macOS | `ipconfig getsummary` of the Wi-Fi device | The keychain, with `security`. macOS asks for your password. |
| Linux | `nmcli dev wifi`, the active line | `nmcli -s` of the connection |
| Windows | `netsh wlan show interfaces` | `netsh wlan show profile ... key=clear` |

`--qr` prints a code in the format `WIFI:T:WPA;S:<name>;P:<password>;;`,
which the camera of a phone reads to join the network.

## odd firewall

`odd firewall open <port> [--udp]`, `close`, `list`. Windows and Linux.

| Host | What it changes |
| :-- | :-- |
| Windows | A rule `odd-<port>-<tcp|udp>` of `netsh advfirewall` |
| Linux with ufw active | `ufw allow <port>/<proto>` |
| Linux with firewalld running | `firewall-cmd --permanent --add-port`, then `--reload` |

On a Linux with no firewall, the port is open already, and `odd` changes
nothing. Undo closes the port.

## odd share

`odd share <path> [--port <n>]` serves a file or a folder until Ctrl+C, at
`http://<address>:<port>/<token>/`, and prints a QR code of the link. The
token is 16 random bytes. A request without it, or for a path outside the
shared folder, gets 404. It answers GET only. It changes nothing on the
computer. On the first run, the firewall of macOS or Windows can ask to let
`odd` take connections.

## odd expose

`odd expose <port> [--public]` runs `tailscale serve <port>`, or
`tailscale funnel <port>` with `--public`. Tailscale prints the URL, and
Ctrl+C stops the share. It needs Tailscale (`odd ssh --tailscale`).

## odd carry

`odd carry send <computer> [<file>...]` packs files of your home directory in
one `tar`, and sends it with Taildrop. With no file, it sends the settings
files that exist: `.ssh/config`, `.gitconfig`, `.gitconfig-*`, `.profile`,
`.zprofile`, `.bashrc`, `.zshrc`. It sends a private key only when you name
it.

`odd carry receive` waits for the files, unpacks them into
`~/.local/state/odd/carry/files`, and lists them. `--apply` copies them into
your home directory. Undo puts back the files before.

## odd remote

| Host | What it turns on | Administrator |
| :-- | :-- | :-- |
| macOS | Screen Sharing (`launchctl load -w` of its daemon) | yes |
| Windows | Remote Desktop (`fDenyTSConnections` = 0) and its firewall rule | yes |
| Linux | GNOME Remote Desktop (`grdctl rdp enable`) | no |

Windows Home has no Remote Desktop server, and `odd` says so. On Linux, set
the user name and the password in Settings, because `grdctl` takes them only
on the command line. Undo turns it off.

## odd font

`odd font <file | folder | archive | name>` installs each `.ttf` and `.otf`
that it finds, for the user only, with no administrator. A name such as
`JetBrainsMono` downloads that Nerd Font from its latest release (`.zip` on
macOS and Windows, `.tar.xz` on Linux), and `tar` unpacks it.

| Host | Where the fonts go | Then |
| :-- | :-- | :-- |
| macOS | `~/Library/Fonts` | |
| Linux | `~/.local/share/fonts` | `fc-cache -f` |
| Windows | `%LOCALAPPDATA%\Microsoft\Windows\Fonts` | a value `<name> (TrueType)` in `HKCU\...\Fonts` |

A font file that is there already stays. Undo deletes the files that `odd`
copied, and their registry values.

## odd devcert

`odd devcert [<name>...]` makes, in `~/.local/share/odd/devcert`:

1. A P-256 authority `odd devcert CA` (`ca.pem`, `ca.key`), one time.
2. `localhost.pem` and `localhost.key` for `localhost`, `127.0.0.1`, `::1` and
   each name that you give, for 825 days.
3. The trust of the host in the authority: the System keychain on macOS, the
   store of the distribution on Linux, the store of the user on Windows
   (Windows asks you).

It uses the `openssl` of the host (Git for Windows has one). Undo removes the
trust, and keeps the files. Firefox has its own store: import `ca.pem` there.

## odd autostart

`odd autostart add <name> -- <command> [args]`, `remove <name>`, `list`.

| Host | What `add` makes |
| :-- | :-- |
| macOS | `~/Library/LaunchAgents/com.odd.<name>.plist`, then `launchctl load -w` |
| Linux | `~/.config/systemd/user/odd-<name>.service`, then `systemctl --user enable --now` |
| Windows | The value `odd-<name>` in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |

Each argument stays whole, with its spaces and quotes. `remove` and undo
reverse it.

## odd cache-purge

`odd cache-purge` lists the caches that exist, with their size and the number
of files, and asks for each one. `--all` deletes each one, `--only <name>`
selects, and `--list` deletes nothing. The caches: npm, pnpm, yarn, pip,
cargo, go, gradle, and on macOS xcode (DerivedData) and homebrew. A link in a
cache goes, and its target stays. Each tool fills its cache again when it
needs it, so there is no undo.

## odd apps

`odd apps init` writes `~/.config/odd/apps.toml` (`%APPDATA%\odd\apps.toml`):

```toml
apps = ["git", "gh", "vscode"]

[custom.myapp]
brew = "myapp"        # or cask = "myapp"
winget = "Vendor.MyApp"
apt = "myapp"
```

`odd apps install` installs each app that is not installed, with Homebrew on
macOS (it offers to install Homebrew), apt, dnf or pacman on Linux (with
`sudo`), and winget on Windows. `odd apps catalog` lists the names that `odd`
knows: git, gh, vscode, firefox, chrome, nodejs, python, rustup, ripgrep, fd,
jq, neovim, tmux, vlc, obsidian. A `[custom.<name>]` table wins over the
catalog.

## odd dualboot-clock

Windows keeps local time in the hardware clock, and Linux keeps UTC, so the
clock jumps by hours after each switch. One side is enough:

- On Windows (the better side): `odd dualboot-clock` sets
  `RealTimeIsUniversal` = 1, from the next restart.
- On Linux: `odd dualboot-clock --linux-local` runs
  `timedatectl set-local-rtc 1`.

Undo puts back the setting before.
