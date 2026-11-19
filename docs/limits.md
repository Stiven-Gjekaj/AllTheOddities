<div align="center">
  <a href="../README.md"><b>AllTheOddities</b></a>
</div>

# Limits

What each oddity cannot do on each host, and what the tests do not cover.

## All oddities

- No unit test touches the real system. The tests give a fake host. So the
  first run on a real computer of each host is the real test, and the label
  stays experimental until that run.
- `odd` reads the network with the `curl` of the host, or `wget`. A Linux
  host with neither cannot read the network.

## odd path

- On macOS and Linux, a new PATH reaches each new login shell. A shell that
  is open already does not see it.
- On Windows, a new `Path` reaches each new window. A window that is open
  already does not see it.

## odd ssh

- **macOS, in:** `systemsetup` can ask for Full Disk Access for the terminal.
  Then `odd` gives the steps in System Settings.
- **macOS, Tailscale:** the first run of Tailscale asks you to allow its VPN
  configuration. `tailscale up` can fail until you allow it. Run
  `odd ssh --tailscale` again after.
- **Linux, in:** `odd` knows the package managers of the Debian, Fedora,
  Arch and openSUSE families. On another distribution, install the SSH
  server first, then run `odd ssh --in`.
- **Linux, keys only:** `odd` checks the config with `/usr/sbin/sshd -t`. A
  distribution that keeps `sshd` elsewhere fails the check, and `odd` puts
  back the config.
- **Keys only:** `odd` writes `ChallengeResponseAuthentication no` and not
  `KbdInteractiveAuthentication no`. Windows 10 has OpenSSH 8.1, which does
  not know the new name. Each later OpenSSH takes the old name.
- **Windows, Tailscale:** Tailscale SSH has no server on Windows, so
  `--tailscale-ssh` does nothing there.
- **Windows, out:** the `ssh-agent` service is off by default. Without an
  administrator, `odd` does not turn it on, and says how.
- **Under sudo:** the out part stops, because the key would belong to root.
  Run it without `sudo`.

## Facts that wait for a real computer

- That `systemsetup -setremotelogin on` works on the current macOS from a
  terminal with Full Disk Access.
- That a file that `curl` or `Invoke-WebRequest` downloads gets no download
  mark, so macOS and SmartScreen do not block `odd`.
- The whole of `odd ssh` on a new Windows 10, Windows 11, macOS and Linux
  computer.

## odd git

- A folder that you move or rename keeps its old path in `~/.gitconfig`. Run
  `odd git add` again for the new path.

## odd tweaks

- On Windows, `odd` writes a DWORD in the form that `reg query` prints, such
  as `0x1`. This waits for a test on a real Windows computer.
- macOS key repeat changes at the next login.

## odd hostname

- On macOS, a name that was not set before (`HostName` often is not) stays
  set after undo.
- On Windows, the new name comes after a restart.

## odd dns

- On Linux, `odd dns` needs NetworkManager, and sets IPv4 servers only.
- On Windows, `odd` reads the static servers from the registry key of the
  interface. A VPN that sets its own servers can hide them.

## odd wifi-pass

- On Windows, `odd` reads the English labels of `netsh` (`SSID`,
  `Key Content`). On Windows in another language, it does not find them.
- On Linux, reading a password can need `sudo`.
- On Windows, a network name with a space waits for a test: `odd` gives
  `netsh` the argument `name=<network>` in one piece.

## odd share

- One file at a time goes in memory before it goes out, so a very large file
  needs as much free memory.
- The link uses HTTP, not HTTPS. The token keeps strangers out, but a device
  on the same network can read the traffic.

## odd carry

- Both computers need Tailscale on the same tailnet. On Linux,
  `tailscale file get` needs `sudo`, or `tailscale set --operator=$USER`.
- The unpacked files stay in the staging folder after a receive.

## odd remote

- macOS can refuse to turn on Screen Sharing from a terminal. Then turn it on
  in System Settings.

## odd font

- An app that is open sees a new font after a restart of the app.

## odd devcert

- Firefox does not read the store of the system. Import `ca.pem` in Firefox,
  or turn on `security.enterprise_roots.enabled`.
- The key of the authority is in your home directory. Anybody who reads it
  can make a certificate that this computer trusts. Run `odd devcert --undo`
  when you no longer need it.

## odd apps

- The catalog is short. Add other apps with `[custom.<name>]`.
- Linux needs `sudo`, and knows the Debian, Fedora and Arch families.

## odd dualboot-clock

- The change on Windows takes effect at the next restart.

## odd autostart

- On Linux, systemd wants the full path of the program, such as
  `/usr/local/bin/sync`. A shell turns `~/bin/sync` into a full path before
  `odd` sees it.
