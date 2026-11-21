# Changelog

Each release of `odd`. [docs/releasing.md](docs/releasing.md) gives the rules
of the versions, the codenames and the labels.

## Unreleased

### Changed

- `odd ssh --tailscale` works on a Linux that the install script of Tailscale
  does not know, such as the SystemRescue live system. `odd` uses the static
  binaries of Tailscale, and starts `tailscaled` when no `tailscaled` runs.

## 2026.11.0 "Astrolabe" (2026-11-19)

The first release: 20 oddities, each one experimental.

### New

- `odd ssh` (experimental): makes SSH ready. It connects out to GitHub, lets
  other computers connect in with keys only, and joins Tailscale with an
  ephemeral key.
- `odd path` (experimental): adds a directory to the PATH of the user for
  good, and lists the PATH.
- `odd git` (experimental): a commit name and email for each folder, and
  signed commits.
- `odd env` (experimental): an environment variable of the user, for good.
- `odd tweaks` (experimental): the developer settings of a fresh computer.
- `odd hostname` (experimental): the name of the computer, in each place.
- `odd hosts` (experimental): entries of the hosts file, in a block of odd.
- `odd dns` (experimental): the DNS servers of the network in use.
- `odd wifi-pass` (experimental): the password of a saved Wi-Fi network, and
  a QR code.
- `odd firewall` (experimental): open or close a port.
- `odd share` (experimental): serve a file or a folder with a secret link.
- `odd expose` (experimental): share a local port on the tailnet or in public.
- `odd carry` (experimental): settings files to another computer, over
  Taildrop.
- `odd remote` (experimental): remote desktop.
- `odd font` (experimental): a font for the user only, or a Nerd Font by
  name.
- `odd devcert` (experimental): a trusted HTTPS certificate for localhost.
- `odd autostart` (experimental): a command at each login.
- `odd cache-purge` (experimental): the caches of developer tools.
- `odd apps` (experimental): the apps of one list.
- `odd dualboot-clock` (experimental): one clock for Windows and Linux.
- `odd list`, `odd update` and `odd --version`.
- The one-line installers `install.sh` and `install.ps1`.

### Removed

- `ssh/new-machine.sh` and `ssh/new-machine.ps1`. `odd ssh --out` does their
  steps.
