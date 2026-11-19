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
