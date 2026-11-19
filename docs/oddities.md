# The oddities

This document says how each oddity works, what it changes on the computer,
and how I tested it.

## ssh/new-machine.sh and ssh/new-machine.ps1

Make a new computer ready to use SSH with GitHub. `new-machine.sh` is for
macOS and Linux. `new-machine.ps1` is for Windows. The two scripts do the same
steps, and the Windows script adds one step for Git.

### What it changes

| Step | What it changes | When it skips the step |
| :-- | :-- | :-- |
| 1 | Makes `~/.ssh`, and sets it to `700` | Never. The step is safe to do again. |
| 2 | Makes an ed25519 key at `~/.ssh/id_ed25519` | The key exists. The script keeps it. |
| 3 | Adds a `Host github.com` block to `~/.ssh/config` | The marker line `# AllTheOddities: github` is in the file. |
| 4 | Gives the key to the agent | You give `-o`, or no agent runs on Linux. |
| 4a | Windows only: sets `core.sshCommand` in the global Git config to the ssh of Windows | You give `-Offline`, Git is not installed, or you set `core.sshCommand` already. |
| 5 | Sends the public key to GitHub with `gh ssh-key add` | You give `-o`, or `gh` is not signed in. Then it prints the key and the link. |
| 6 | Connects to `git@github.com` to test the key | You give `-o`. |

The script adds to `~/.ssh/config`. It does not replace the entries that are
there.

### Each host

| Host | The agent | The config block |
| :-- | :-- | :-- |
| macOS | `ssh-add --apple-use-keychain`. The keychain keeps the passphrase after a restart. | Adds `UseKeychain yes`. |
| Linux | `ssh-add`, if the environment has `SSH_AUTH_SOCK`. | No `UseKeychain`, because OpenSSH on Linux does not know it. |
| Windows | The `ssh-agent` service. Windows turns it off by default. As an administrator, the script sets it to start with Windows and starts it. Without an administrator, it tells you to run the script one time as one. | No `UseKeychain`. The path of the key uses forward slashes. The script writes UTF-8 with no BOM. |

On Windows, the script needs the OpenSSH Client. Windows 10 since 1809 and
Windows 11 have it. If `ssh-keygen` is not found, add the OpenSSH Client in
Settings > System > Optional features.

Git for Windows has its own ssh. That ssh does not use the `ssh-agent`
service, so Git asks for the passphrase on each push. Step 4a fixes this.

On the first connection, `StrictHostKeyChecking=accept-new` adds the host key
of GitHub to `~/.ssh/known_hosts`. Compare the fingerprint with the one that
GitHub publishes at
<https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints>.

### The test

```bash
sh ssh/test-new-machine.sh
```

```powershell
./ssh/test-new-machine.ps1
```

Each test runs its script in offline mode, in a temporary home directory. It
does not touch your `~/.ssh`. It checks that:

- the key and the public key exist, and the key has the comment;
- `~/.ssh` is `700`, and the key and the config are `600` (the `.sh` test only);
- the config has no BOM (the `.ps1` test only);
- an entry that was in the config before is still there, also when its last
  line has no newline (the `.ps1` test);
- a second run keeps the same key and adds no second block;
- `ssh -G` reads the path of the key from the config.

I ran both tests on macOS: the `.sh` test with `sh`, and the `.ps1` test with
PowerShell 7.7. CI runs the `.sh` test on Linux and macOS, and the `.ps1` test
on Windows in Windows PowerShell 5.1 and in PowerShell 7.

The steps after the config need the network and a GitHub account, so no test
runs them. I did not run the Windows script on a real Windows computer yet.
