# Plans 2 to 4: the calls made during the build

The agent made these calls while it built plans 2, 3 and 4. Each line says
what it decided and why. The owner can change each one.

## Plan 2

- Task 1: complete (a96ddf6). Ruling: the folder is canonicalized (links resolved), because git matches gitdir against the resolved path. Ruling: the profile file name keeps only letters, digits, '.', '_' and '-' of the last part of the folder.
- Task 2: complete (bbc3482). Ruling: Windows writes REG_SZ, not REG_EXPAND_SZ, so a % in a value stays as it is. Ruling: on Windows, list reads the names from the record of odd.
- Task 3: complete (15e330a). Ruling: a DWORD is written in the form reg query prints (0x1), so undo can write back what it read. Cost if wrong: reg add refuses 0x1 on some Windows; CI does not run reg.
- Task 4: complete (4c787a9). Ruling: on Linux only 127.0.1.1 lines of /etc/hosts change, and only the word that equals the old name.
- Task 6: complete (f1cda20). Ruling: on Linux odd dns sets IPv4 servers only (ipv4.dns); an IPv6 address exits 2. Ruling: on Linux, servers count as DHCP when ipv4.ignore-auto-dns is no.
- Task 7: complete. Plan 2 CI green at b7520a8. Ruling: full_folder follows the real filesystem (backslashes become slashes on each host), fix commit b7520a8.

## Plan 3

- Task 0: complete (ad4f614). Ruling: light modules and the quiet zone are filled, so a dark terminal shows a normal code (as qrencode -t UTF8). Cost if wrong: on a light terminal the code shows inverted; most phones read it.
- Task 1: complete (08e0e46). Ruling: macOS reads the SSID with 'ipconfig getsummary', because 'networksetup -getairportnetwork' hides it on new macOS. Ruling: Windows reads the English labels of netsh (SSID, Key Content); limits.md names this.
- Task 2: complete (b16f10c). Note: list_shows_the_open_ports was written after the code and passed at once. Ruling: ufw comes before firewalld when both run.
- Task 3: complete (7adb8ef). Ruling: serve() canonicalizes the root itself, so the inside-the-root check compares real paths. Checked by hand: a real share served a file, and gave 404 to ../ and to a request with no token. Ruling: the server runs until the process ends (Ctrl+C), with no stop flag.
- Task 4: complete (bf98a19). Ruling: tailscale serve and funnel run on the terminal of the person (interactive), so Tailscale prints the URL itself and Ctrl+C stops it.
- Task 5: complete (db79f50). Ruling: files go as one tar, so their paths survive Taildrop; receive unpacks into ~/.local/state/odd/carry/files and copies only with --apply. Ruling: a file that is not text is not copied by --apply. minor (deferred): the unpacked files stay in the staging folder after a receive.
- Task 6: complete (94da641). Ruling: Linux does not set the RDP credentials, because grdctl takes them only as arguments; odd tells the person to set them in Settings.

## Plan 4

- Task 0: complete (54a34aa). Ruling: remove_dir uses std remove_dir_all, which deletes a link and does not follow it.
- Task 1: complete (9ca2cf1). Ruling: the font argument is optional for clap, so --undo parses; run asks for it.
- Task 2: complete (c01deca). Checked by hand: the openssl commands make a chain that 'openssl verify' accepts, with OpenSSL 4.1 and with the LibreSSL 3.3.6 of macOS. Ruling: the leaf also gets keyUsage=critical,digitalSignature. Ruling: undo removes the trust and keeps the files.
- Task 3: complete (5e73888). Ruling: Windows uses the HKCU Run key, not schtasks /SC ONLOGON, which needs an administrator.
- Task 4: complete (58ae8f3). Checked by hand: --list on this Mac lists 8 caches and deletes nothing.
- Task 5: complete (fcc54a6). Ruling: the catalog has 15 apps; tailscale is out, because odd ssh --tailscale installs it, and its Homebrew name is not certain. Ruling: installs run on the terminal (interactive), so a manager can ask its questions; apt-get update runs once before the first install.

## Fixed in the review

- odd devcert under sudo: the files go to the person who ran sudo (test under_sudo_the_files_go_to_the_person, RED then GREEN).
- The installer tests: the bad checksum is now always wrong (it was valid one time in 16).

## Review

The review was a self-review by the agent, not a fresh reviewer: the CLAUDE.md
of the owner forbids background subagents.
