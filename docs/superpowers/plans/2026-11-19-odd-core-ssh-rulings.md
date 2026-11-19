# Plan 1: the calls made during the build

The agent made these calls while it built plan 1
(`2026-11-19-odd-core-ssh.md`). Each line says what it decided, why, and
what it costs if it is wrong. The owner can change each one.

## Rulings

- Ruling: work on branch odd-core in the main checkout, not a worktree. The user said "build everything"; a branch keeps main clean. Cost if wrong: none, the branch merges as normal.
- - T5 Undo::Call -> T6 Oddity::undo_call: T5 comes before the trait. Ruling: state::undo takes a callback `&mut dyn FnMut(&str, &str, &mut Ctx) -> Result<()>` for Call steps; T6 dispatch passes the oddity's undo_call. Cost if wrong: one signature change.
- - T7 same_entry -> T13: the installers are shell, they do not call Rust. Ruling: T13 calls `odd path add`, which uses same_entry. No conflict.
- Task 1: Ruling: LONG is a const made with concat! and a codename! macro, because clap 4.6 takes only a &'static str for the version without the 'string' feature. Cost if wrong: none.
- Task 2: Ruling: Scripted (the test Ask) lives in testing.rs from Task 2, because TestCtx needs an Ask. Task 3 tests it there. Cost if wrong: none.
- Task 2: Ruling: FakeRunner answers by program and argument prefix; several answers for one call go in order, the last one stays. Not in the plan; it lets a test give a first and a second answer. Cost if wrong: none.
- Task 3: Ruling: the plan's open_input(bool) became pick_source(stdin_is_terminal, tty_opens), so the test does not need a real terminal. Cost if wrong: none.
- Task 4: Ruling: with no final newline, the block starts on the next line with no blank line before it, unlike the plan's 'then a blank line'. Only so can remove() give back the text exactly ('a' and 'a
- Task 5: Ruling: a second RestoreFile for the same path keeps the first 'before' and takes the new checksum, so undo goes back to the text before odd, not to the text of the first run. Cost if wrong: none.
- Task 5: Ruling: added steps() (read the record) and Ctx::remove_file (dry-run aware); the plan did not name them, the tests and undo need them. Cost if wrong: none.
- Task 6: Ruling: dispatch takes the slice of oddities as a third argument, so a test gives a test oddity. Cost if wrong: none.
- Task 6: Ruling: every oddity parses --undo; one that cannot undo prints 'nothing to undo' and exits 2, not a parser error. Cost if wrong: none.
- Task 6: Ruling: an undo that skips a step exits 1, because a change stays on the host. Cost if wrong: a script sees 1 where it expected 0.
- Task 6: Ruling: the 'No terminal' note prints on the first question only, not at each start. Cost if wrong: none.
- Task 7: Ruling: Windows reads and writes the user Path with reg.exe through the Runner, not the registry API, so the tests run on each host. Only the WM_SETTINGCHANGE broadcast uses windows-sys. Cost if wrong: reg.exe quoting of an odd path; the CI on Windows will show it.
- Task 7: Ruling: bash also gets ~/.bash_profile when it exists, because bash then ignores ~/.profile. Cost if wrong: one more block.
- Task 7: Ruling: a profile that odd creates is recorded as RestoreFile(before: None), so undo deletes it; a file of the person gets RemoveBlock. Cost if wrong: none.
- Task 7: Ruling: fish gets the directory inside fish single quotes, not as an extra argument. Cost if wrong: none.
- Task 7: Ruling: added block::body() to read the dirs of the block. Cost if wrong: none.
- Task 7: Ruling: the common flags are global in clap, so 'odd path add /x --dry-run' works (fix commit f6a97df, test dry_run_after_add_changes_nothing RED->GREEN). Cost if wrong: none.
- Task 8: Ruling: added Runner::interactive and Ctx::interactive, so ssh-keygen and ssh-add ask for the passphrase on the terminal. odd never sees the passphrase, and none goes on a command line. Cost if wrong: none.
- Task 8: Ruling: FakeRunner now picks the longest matching prefix, then the newest answer, and keeps answers (Task 2 used order). A test changes one answer of a shared set. Cost if wrong: none.
- Task 8: Ruling: added state::upsert_block (block + the correct undo step), and odd path uses it too. Cost if wrong: none.
- Task 8: Ruling: the test of GitHub uses BatchMode=yes, so it never waits for a question. Cost if wrong: a key with a passphrase and no agent fails the test, and the output says why.
- Task 8: Ruling: the agent step on Windows does not record an undo: the spec gives undo for the config block and the Git setting only. Cost if wrong: the agent service stays on after undo.
- Task 9: Ruling: the capability state is read with one PowerShell line (CAPABILITY_STATE), and the server installs only when it is not 'Installed'. Cost if wrong: none.
- Task 9: Ruling: on Linux odd first looks for the ssh or sshd unit, and installs the package only when neither exists. Cost if wrong: none.
- Task 9: Ruling: the undo of the Windows service sets it back to 'demand' (start by hand), as Windows installs it, and stops it. The package or the Windows feature stays installed after undo. Cost if wrong: the server program stays on the disk, turned off.
- Task 9: Ruling: macOS undo uses 'systemsetup -f -setremotelogin off', so it does not stop on a question. Cost if wrong: none.
- Task 10: Ruling: no ureq. HostHttp runs the curl of the host (wget when there is no curl), HTTPS only. ureq's TLS library compiles C for each target, which breaks the musl and cross builds that AllTheTools keeps free of C. Cost if wrong: a Linux host with neither curl nor wget cannot read the network; odd says so.
- Task 10: Ruling: added Ctx::root and Ctx::sys_path, so a test puts system files such as administrators_authorized_keys in its temporary directory. server::config_path now takes the Ctx. Cost if wrong: none.
- Task 10: Ruling: under sudo on macOS and Linux, odd uses the home of SUDO_USER, chowns the authorized_keys file and its directory to that user, and the out part stops (exit 2) so the key does not belong to root. Cost if wrong: an odd sudo setup can give a wrong home; the files then land in /root.
- Task 11: Ruling: 'ChallengeResponseAuthentication no' in place of the spec's 'KbdInteractiveAuthentication no'. Windows 10 ships OpenSSH 8.1, which does not know the new name, so sshd -t would fail there; each later OpenSSH takes the old name as an alias. Cost if wrong: a future OpenSSH that drops the alias fails sshd -t, and odd puts back the config.
- Task 11: Ruling: the undo step is recorded after sshd -t passes, not before the write, so a config that odd put back leaves no step to undo. Cost if wrong: a crash between the write and the record leaves a change with no undo step.
- Task 11: Ruling: sshd -t runs /usr/sbin/sshd on macOS and Linux (usr-merged distributions have it there too). Cost if wrong: a distribution with sshd elsewhere fails the check and puts back the config.
- Task 11: Ruling: with no GitHub user, the count is the keys already in the file, so a file with keys still gets keys-only. Cost if wrong: none.
- Task 12: Ruling: the macOS package puts the CLI in /Applications/Tailscale.app/Contents/MacOS/Tailscale and not on PATH, and Windows in C:\Program Files\Tailscale; odd looks there. After the macOS install, odd opens the app and tells the person to allow the VPN configuration. Cost if wrong: tailscale up can fail on the first run until the person allows it.
- Task 12: Ruling: the key file lives in a directory under the state directory (mode 700, file 600; icacls on Windows), not in the system temp directory, so a test can see that it is gone. Cost if wrong: none.
- Task 12: Ruling: TS_AUTHKEY is read in each mode, not only with --yes. Cost if wrong: none.
- Task 12: Ruling: the step that removes the scripts also removes ci.yml, which ran only them; Task 15 writes the new one. Cost if wrong: no CI on this branch until Task 15.
- Task 13: Ruling: install.ps1 takes ODD_NO_PATH=1 to skip 'odd path add', so the Windows CI test does not change the PATH of the runner. Cost if wrong: none; it is also of use to a person who keeps PATH by hand.
- Task 13: Ruling: install.ps1 downloads and checks in a function, and runs odd at the top level, because a PowerShell function returns each line that a native command prints. It exits only when it runs as a file, so 'irm | iex' does not close the window. Checked by hand: the window stays.
- Task 13: Ruling: the ps1 test checks the version output only with the real odd.exe (Windows CI). On macOS the fake cannot run, because the download loses the mode that lets it run. Cost if wrong: the run-once output on Windows is checked only in CI.
- Task 14: Ruling: update::run takes the path of the binary; dispatch passes current_exe, a test passes a file in its temporary directory. build.rs gives ODD_TARGET (not TARGET, which Cargo sets for build scripts only). Cost if wrong: none.
- Task 15: Ruling: smoke.sh also runs 'path list --json'-free reads and 'path add --dry-run'; 'odd update --dry-run' runs but does not fail the smoke until the first release exists. Cost if wrong: a broken update shows only after the first release.
- Task 15: Ruling: the release has no Debian or RPM step and names the files without the version, so releases/latest/download/<file> works for the installers and odd update. Cost if wrong: none.

## Fixed in the final review

- keys-only could stay off behind 50-cloud-init.conf (sshd keeps the first value) - drop-in renamed 00-odd.conf and odd checks sshd -T; another_file_that_turns_passwords_on_puts_back_the_config RED->GREEN, suite 117/117. The rename itself went in with the test edit, so the_drop_in_sorts_before_the_files_of_the_distribution never ran red.
- the record under sudo belonged to root - save_as_person chowns the new directories; under_sudo_the_record_belongs_to_the_person RED->GREEN, suite 119/119.

## Deferred minors

- in dry-run, odd path still prints 'is on PATH in each new shell'.
- in dry-run, success lines such as 'The key is on GitHub' still print.
- with no release yet, odd update prints the 404 of curl, not 'no release yet'.
- authorized_keys lines are added with LF, also in a Windows file with CRLF; sshd reads both.
- an undo under sudo saves the kept steps without the chown.

## Review

The final review was a self-review by the agent, not a fresh reviewer: the
CLAUDE.md of the owner forbids background subagents. A fresh review is still
open.
