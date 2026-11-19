<div align="center">

![AllTheOddities](assets/banner.svg)

### The small fixes that helped once, and will help again

_One command, `odd`. The same oddity on Windows, on macOS and on Linux._

![Rust](https://img.shields.io/badge/rust-facc15?style=for-the-badge&logo=rust&logoColor=030712)
![Windows](https://img.shields.io/badge/windows-fb923c?style=for-the-badge&logo=windows&logoColor=030712)
![macOS](https://img.shields.io/badge/macos-f472b6?style=for-the-badge&logo=apple&logoColor=030712)
![Linux](https://img.shields.io/badge/linux-e879f9?style=for-the-badge&logo=linux&logoColor=030712)
[![MIT licence](https://img.shields.io/badge/mit_licence-c084fc?style=for-the-badge&logoColor=030712)](LICENSE)

<p align="center">
  <a href="#overview"><b>Overview</b></a> |
  <a href="#install"><b>Install</b></a> |
  <a href="#the-oddities"><b>Oddities</b></a> |
  <a href="#odd-ssh"><b>SSH</b></a> |
  <a href="#for-a-script"><b>Scripts</b></a> |
  <a href="docs/oddities.md"><b>Each oddity</b></a>
</p>

</div>

---

> [!NOTE]
> **Before the first release.**
> The 20 oddities of the first release, `2026.11.0 "Astrolabe"`, are in `odd`.
> Each one is **experimental**: it passes its tests on Windows, macOS and
> Linux against a fake host, and waits for a run on real computers. Its flags
> and its output can change. [docs/releasing.md](docs/releasing.md) gives the
> rules of the versions and the labels.

---

## Overview

**AllTheOddities** is one command, `odd`, with small fixes in it that each
helped once. It is the sibling of
[AllTheTools](https://github.com/Stiven-Gjekaj/AllTheTools).

AllTheTools holds the tools that many people need many times. This repository
holds the niche jobs: the steps that make a new computer ready, and the fix
for one odd problem that you otherwise find again with a search each time,
on three systems with three different answers.

<table>
<tr>
<td width="50%" valign="top">

### What it aims to do

- **Keep the fix**, so that you do not search for it again.
- **Read the host**, and do the correct steps for it.
- **Run again safely.** It skips each step that is done already.
- **Undo what it did.** `--undo` reverts each change that `odd` recorded.

</td>
<td width="50%" valign="top">

### How it aims to behave

- `--dry-run` prints each step and each command, and changes nothing.
- It stops before the first change when a step needs an administrator.
- It never puts a secret on a command line, in a file of its own, or in
  its output.
- No test touches the real system of the person who runs it.

</td>
</tr>
</table>

---

## Install

On macOS or on Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.sh | sh
```

On Windows, in Windows PowerShell or in PowerShell 7:

```powershell
irm https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.ps1 | iex
```

The installer finds your system and your processor, downloads the binary of
the latest release, and checks it against `SHA256SUMS`. A binary that does not
match stops the install. Then it puts `odd` in `~/.local/bin`, or in
`%LOCALAPPDATA%\Programs\odd`, and adds that directory to your PATH.

### Run one time

On a computer that you only set up, run `odd` one time and install nothing:

```bash
curl -fsSL https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.sh | sh -s -- ssh
```

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.ps1))) ssh
```

Each installer is short. Read it before you run it, if you like:
[install.sh](install.sh), [install.ps1](install.ps1).
`odd update` replaces `odd` with the latest release later, with the same
check.

---

## The oddities

| Oddity | What it does | Hosts | Label |
| :-- | :-- | :-- | :-- |
| `odd ssh` | Makes SSH ready: connect out to GitHub, let others connect in, and join Tailscale | all | experimental |
| `odd path` | Adds a directory to the PATH of the user for good, and lists the PATH | all | experimental |
| `odd git` | Sets the commit name and email for each folder, and signs commits with the SSH key | all | experimental |
| `odd env` | Sets an environment variable of the user for good | all | experimental |
| `odd tweaks` | Applies the developer settings of a fresh computer, such as hidden files and long paths | all | experimental |
| `odd hostname` | Sets the name of the computer in each place that the host keeps it | all | experimental |
| `odd hosts` | Adds and removes entries of the hosts file, in a block that `odd` manages | all | experimental |
| `odd dns` | Sets the DNS servers of the network in use, with presets | all | experimental |
| `odd wifi-pass` | Shows the password of a saved Wi-Fi network, and a QR code for a phone | all | experimental |
| `odd firewall` | Opens or closes a port in the firewall | Windows, Linux | experimental |
| `odd share` | Serves a file or a folder on the local network with a secret link and a QR code, until Ctrl+C | all | experimental |
| `odd expose` | Shares a local port on your tailnet, or in public, until Ctrl+C | all | experimental |
| `odd carry` | Sends settings files to another computer of your tailnet | all | experimental |
| `odd remote` | Turns on remote desktop | all | experimental |
| `odd font` | Installs a font for the user only, or a Nerd Font by name | all | experimental |
| `odd devcert` | Makes a trusted HTTPS certificate for localhost | all | experimental |
| `odd autostart` | Starts a command when you log in | all | experimental |
| `odd cache-purge` | Lists the caches of developer tools with their size, and deletes the ones you select | all | experimental |
| `odd apps` | Installs the apps of one list with the package manager of the host | all | experimental |
| `odd dualboot-clock` | Stops the clock from jumping between Windows and Linux | Windows, Linux | experimental |

`odd list` shows each oddity of your build. `odd <oddity> --help` gives its
options. [docs/oddities.md](docs/oddities.md) says what each one changes on
each host.

### odd ssh

```bash
odd ssh
```

It asks three questions, and does each part that you say yes to:

1. **Out: connect from this computer to GitHub.** It makes an ed25519 key,
   adds a GitHub block to `~/.ssh/config`, gives the key to the agent, sends
   the public key to GitHub with `gh` when `gh` is signed in, and tests the
   connection. On Windows, it also makes Git use the ssh of Windows, which
   uses the agent.
2. **In: let other computers connect to this one.** It turns on the SSH
   server, adds the public keys of a GitHub user, and then turns off password
   login, so that only those keys can log in. It turns off passwords only
   when at least one key can log in, and it checks the new config with
   `sshd -t` before it restarts the server.
3. **Tailscale: reach this computer from anywhere.** It installs Tailscale if
   it is missing, and joins your tailnet with an ephemeral auth key. The
   computer leaves the tailnet soon after it goes offline.

```bash
odd ssh --out                   # only the first part, with no question
sudo odd ssh --in --github-user you
sudo odd ssh --tailscale        # reads TS_AUTHKEY, or asks with hidden input
odd ssh --undo                  # turns off what odd ssh turned on
```

Run the out part without `sudo`, so that the key belongs to you. The in part
and the Tailscale part need an administrator.

### odd path

```bash
odd path add ~/tools/bin
odd path list
odd path --undo
```

### The others

```bash
odd git add ~/work --name "Ann Lee" --email ann@work.example --sign
odd env set EDITOR nvim
odd tweaks                      # the tweaks of this host, on or off
odd tweaks apply --user-only    # the ones that need no administrator
sudo odd hostname studio
sudo odd hosts add 10.0.0.5 nas nas.lan
sudo odd dns set cloudflare
sudo odd dns reset              # back to the servers of DHCP
odd wifi-pass --qr              # the current network, and a QR code
sudo odd firewall open 8080
odd share ~/Downloads/build.zip # a link and a QR code for a phone
odd expose 3000 --public        # a dev server on the internet, with Tailscale Funnel
odd carry send laptop           # your settings files, over Taildrop
odd carry receive --apply       # on the laptop
sudo odd remote
odd font JetBrainsMono          # a Nerd Font, by name
sudo odd devcert myapp.test     # https://localhost and https://myapp.test
odd autostart add sync -- ~/bin/sync --quiet
odd cache-purge                 # asks for each cache, with its size
odd apps init && odd apps install
odd dualboot-clock              # on the Windows side of a dual boot
```

Each one takes `--dry-run` and `--undo`.

---

## For a script

Each oddity takes these flags:

| Flag | What it does |
| :-- | :-- |
| `--yes` | Takes the default of each question, and asks nothing |
| `--dry-run` | Prints each step and each command, and changes nothing |
| `--undo` | Reverts the changes that the runs of this oddity recorded |

`odd list --json` prints each oddity with its `name`, `label`, `hosts` and
`summary`.

The exit codes:

| Code | Meaning |
| :-- | :-- |
| 0 | The oddity did its work. |
| 1 | A step failed. The output says which step, and what to do. |
| 2 | The usage is not correct, or the oddity does not run on this host. |
| 3 | The oddity needs an administrator. It changed nothing. |

---

## Project documents

| Document | What it holds |
| :-- | :-- |
| [docs/oddities.md](docs/oddities.md) | What each oddity changes on each host, and what undo does |
| [docs/limits.md](docs/limits.md) | What each oddity cannot do on each host |
| [docs/releasing.md](docs/releasing.md) | The versions, the codenames, the labels, and how to cut a release |
| [CHANGELOG.md](CHANGELOG.md) | Each release |
| [AGENTS.md](AGENTS.md) | The rules for anybody who changes this repository, human or agent |
| [docs/superpowers/specs](docs/superpowers/specs) | The design of `odd` |

---

## Licence

MIT. See [LICENSE](LICENSE).

<div align="center">
<sub>Read <code>odd &lt;oddity&gt; --dry-run</code> before you run an oddity that changes your system.</sub>
</div>
