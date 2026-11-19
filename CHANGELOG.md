# Changelog

Each release of `odd`. [docs/releasing.md](docs/releasing.md) gives the rules
of the versions, the codenames and the labels.

## Unreleased

### New

- `odd ssh` (experimental): makes SSH ready. It connects out to GitHub, lets
  other computers connect in with keys only, and joins Tailscale with an
  ephemeral key.
- `odd path` (experimental): adds a directory to the PATH of the user for
  good, and lists the PATH.
- `odd list`, `odd update` and `odd --version`.
- The one-line installers `install.sh` and `install.ps1`.

### Removed

- `ssh/new-machine.sh` and `ssh/new-machine.ps1`. `odd ssh --out` does their
  steps.
