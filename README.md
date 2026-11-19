<div align="center">

![AllTheOddities](assets/banner.svg)

### The small fixes that helped once, and will help again

_Scripts for one job. Each one is short, tested, and safe to run again._

![Shell](https://img.shields.io/badge/shell-facc15?style=for-the-badge&logo=gnubash&logoColor=030712)
![macOS](https://img.shields.io/badge/macos-fb923c?style=for-the-badge&logo=apple&logoColor=030712)
![Linux](https://img.shields.io/badge/linux-f472b6?style=for-the-badge&logo=linux&logoColor=030712)
[![MIT licence](https://img.shields.io/badge/mit_licence-c084fc?style=for-the-badge&logoColor=030712)](LICENSE)

<p align="center">
  <a href="#overview"><b>Overview</b></a> |
  <a href="#the-oddities"><b>Oddities</b></a> |
  <a href="#ssh"><b>SSH</b></a> |
  <a href="#add-an-oddity"><b>Add one</b></a> |
  <a href="docs/oddities.md"><b>Each oddity</b></a>
</p>

</div>

---

> [!NOTE]
> **One oddity so far.**
> This repository grows each time a niche problem costs me an afternoon.
> [docs/oddities.md](docs/oddities.md) says how each oddity works, and on
> which hosts I tested it.

---

## Overview

**AllTheOddities** is the sibling of
[AllTheTools](https://github.com/Stiven-Gjekaj/AllTheTools).

AllTheTools holds the tools that each computer needs, in one binary, with the
same command on each host. A thing goes there when many people need it, many
times.

This repository holds the other things: a script that you run one time on a
new computer, the fix for one odd problem, the note that you wanted the last
time. A thing goes here when it helped once, and it is too niche for `att`.

<table>
<tr>
<td width="50%" valign="top">

### What it aims to do

- **Keep the fix**, so that you do not find it again in a search.
- **Do one job** in each script.
- **Run again safely.** The script skips each step that it did before.
- **Need nothing else** than the tools that the host has.

</td>
<td width="50%" valign="top">

### How it aims to behave

- Each script tells what it does in a comment at the top.
- Each script has a test, and the test never touches your real files.
- It keeps what you have already. It adds and does not replace.
- Each message says what went wrong, and then what to do about it.

</td>
</tr>
</table>

---

## The oddities

| Oddity | What it does | Hosts |
| :-- | :-- | :-- |
| [`ssh/new-machine.sh`](ssh/new-machine.sh) | Makes a new computer ready to use SSH with GitHub | macOS, Linux |

[docs/oddities.md](docs/oddities.md) says how each one works on each host.

### SSH

On the new computer:

```bash
git clone https://github.com/Stiven-Gjekaj/AllTheOddities.git
```

```bash
sh AllTheOddities/ssh/new-machine.sh
```

```
Key made: /Users/you/.ssh/id_ed25519
The GitHub block is added to /Users/you/.ssh/config.
Identity added: /Users/you/.ssh/id_ed25519 (you@new-mac)
The key is on GitHub with the title "new-mac".
GitHub accepts the key.
```

The clone uses HTTPS, because the computer has no key yet. After the script,
SSH works.

| Option | What it does |
| :-- | :-- |
| `-c comment` | Sets the comment of the key. The default is `user@host`. |
| `-k keyfile` | Sets the path of the key. The default is `~/.ssh/id_ed25519`. |
| `-N` | Makes the key with no passphrase. |
| `-o` | Offline. It does not use the agent, `gh`, or the network. |

The exit codes:

| Code | Meaning |
| :-- | :-- |
| 0 | The computer is ready, and GitHub accepts the key. |
| 1 | GitHub does not accept the key yet. The output says why. |
| 2 | An option is not correct. |

---

## Add an oddity

1. Put it in a directory for its topic, such as `ssh/` or `macos/`.
2. Tell what it does, and how to use it, in a comment at the top.
3. Add a test beside it. The test makes its own state, in a temporary
   directory.
4. Add one row to [the oddities](#the-oddities), and one section to
   [docs/oddities.md](docs/oddities.md), in a different commit.

[AGENTS.md](AGENTS.md) gives the rules for each change, human or agent.

---

## Project documents

| Document | What it holds |
| :-- | :-- |
| [docs/oddities.md](docs/oddities.md) | How each oddity works on each host, and how I tested it |
| [AGENTS.md](AGENTS.md) | The rules for anybody who changes this repository, human or agent |
| [scripts/make-banner.py](scripts/make-banner.py) | The script that draws the banner |

---

## Licence

MIT. See [LICENSE](LICENSE).

<div align="center">
<sub>Read a script before you run it. Each one tells what it changes in the comment at the top.</sub>
</div>
