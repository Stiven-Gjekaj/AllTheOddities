# The oddities

This document says how each oddity works, what it changes on the computer,
and how I tested it.

## ssh/new-machine.sh

Makes a new computer ready to use SSH with GitHub.

### What it changes

| Step | What it changes | When it skips the step |
| :-- | :-- | :-- |
| 1 | Makes `~/.ssh`, and sets it to `700` | Never. The step is safe to do again. |
| 2 | Makes an ed25519 key at `~/.ssh/id_ed25519` | The key exists. The script keeps it. |
| 3 | Adds a `Host github.com` block to `~/.ssh/config` | The marker line `# AllTheOddities: github` is in the file. |
| 4 | Gives the key to the agent | You give `-o`, or no agent runs on Linux. |
| 5 | Sends the public key to GitHub with `gh ssh-key add` | You give `-o`, or `gh` is not signed in. Then it prints the key and the link. |
| 6 | Connects to `git@github.com` to test the key | You give `-o`. |

The script adds to `~/.ssh/config`. It does not replace the entries that are
there.

### Each host

| Host | The agent | The config block |
| :-- | :-- | :-- |
| macOS | `ssh-add --apple-use-keychain`. The keychain keeps the passphrase after a restart. | Adds `UseKeychain yes`. |
| Linux | `ssh-add`, if the environment has `SSH_AUTH_SOCK`. | No `UseKeychain`, because OpenSSH on Linux does not know it. |

On the first connection, `StrictHostKeyChecking=accept-new` adds the host key
of GitHub to `~/.ssh/known_hosts`. Compare the fingerprint with the one that
GitHub publishes at
<https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints>.

### The test

```bash
sh ssh/test-new-machine.sh
```

The test runs the script in offline mode, in a temporary home directory. It
does not touch your `~/.ssh`. It checks that:

- the key and the public key exist, and the key has the comment;
- `~/.ssh` is `700`, and the key and the config are `600`;
- an entry that was in the config before is still there;
- a second run keeps the same key and adds no second block;
- `ssh -G` reads the path of the key from the config.

I tested it on macOS. Steps 4 to 6 need the network and a GitHub account, so the
test does not run them.
