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
2. It reads the auth key from `TS_AUTHKEY`, or asks for it with hidden input.
   Make the key **ephemeral** and single-use at
   <https://login.tailscale.com/admin/settings/keys>. An ephemeral computer
   leaves the tailnet soon after it goes offline.
3. It writes the key to a file that only you can read, runs
   `tailscale up --auth-key=file:<path>`, and deletes the file at once. The
   key never goes on a command line, where the list of processes shows it.
4. It prints the tailnet name and address of the computer, and the command
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
