#!/bin/sh
# Makes a new computer ready to use SSH with GitHub.
# It runs on macOS and on Linux. You can run it again: each step that is
# done already is skipped.
#
# The steps:
#   1. Make ~/.ssh with the correct permissions.
#   2. Make an ed25519 key, if the key does not exist.
#   3. Add a GitHub block to ~/.ssh/config.
#   4. Give the key to the SSH agent (on macOS, also to the keychain).
#   5. Send the public key to GitHub with `gh`, if `gh` is signed in.
#   6. Test the connection to GitHub.
#
# Usage: new-machine.sh [-c comment] [-k keyfile] [-N] [-o]
#   -c  The comment of the key. The default is user@host.
#   -k  The path of the key. The default is ~/.ssh/id_ed25519.
#   -N  Make the key with no passphrase.
#   -o  Offline. Do not use the agent, gh, or the network.

set -eu

comment="$(id -un)@$(hostname -s 2>/dev/null || hostname)"
key="$HOME/.ssh/id_ed25519"
no_passphrase=0
offline=0

while getopts c:k:No opt; do
    case "$opt" in
        c) comment=$OPTARG ;;
        k) key=$OPTARG ;;
        N) no_passphrase=1 ;;
        o) offline=1 ;;
        *) sed -n '14,18p' "$0" >&2; exit 2 ;;
    esac
done

say() { printf '%s\n' "$*"; }
ssh_dir="$HOME/.ssh"
config="$ssh_dir/config"
marker="# AllTheOddments: github"

# 1. The directory. SSH refuses keys in a directory that others can read.
mkdir -p "$ssh_dir"
chmod 700 "$ssh_dir"

# 2. The key.
if [ -f "$key" ]; then
    say "Key exists: $key. It is kept."
else
    if [ "$no_passphrase" -eq 1 ]; then
        ssh-keygen -q -t ed25519 -C "$comment" -f "$key" -N ""
    else
        ssh-keygen -q -t ed25519 -C "$comment" -f "$key"
    fi
    say "Key made: $key"
fi
chmod 600 "$key"
chmod 644 "$key.pub"

# 3. The config block. The marker line shows that the block is there.
touch "$config"
chmod 600 "$config"
if grep -qF "$marker" "$config"; then
    say "The GitHub block is in $config already."
else
    {
        [ -s "$config" ] && printf '\n'
        say "$marker"
        say "Host github.com"
        say "    HostName github.com"
        say "    User git"
        say "    IdentityFile $key"
        say "    IdentitiesOnly yes"
        say "    AddKeysToAgent yes"
        if [ "$(uname -s)" = Darwin ]; then
            say "    UseKeychain yes"
        fi
    } >>"$config"
    say "The GitHub block is added to $config."
fi

if [ "$offline" -eq 1 ]; then
    say "Offline: the agent, gh and the test are skipped."
    exit 0
fi

# 4. The agent.
if [ "$(uname -s)" = Darwin ]; then
    ssh-add --apple-use-keychain "$key" || say "The agent did not take the key."
elif [ -n "${SSH_AUTH_SOCK:-}" ]; then
    ssh-add "$key" || say "The agent did not take the key."
else
    say "No SSH agent runs. Start one with: eval \"\$(ssh-agent -s)\""
fi

# 5. GitHub. A key that GitHub has already gives an error, which is not a fault.
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    title="$(hostname -s 2>/dev/null || hostname)"
    if gh ssh-key add "$key.pub" --title "$title" 2>/dev/null; then
        say "The key is on GitHub with the title \"$title\"."
    else
        say "gh did not add the key. GitHub can have it already."
    fi
else
    say "gh is not signed in. Add this key at https://github.com/settings/ssh/new:"
    cat "$key.pub"
fi

# 6. The test. GitHub ends the session with exit code 1 when the key is good.
set +e
out=$(ssh -o StrictHostKeyChecking=accept-new -T git@github.com 2>&1)
set -e
case "$out" in
    *"successfully authenticated"*) say "GitHub accepts the key." ;;
    *) say "GitHub does not accept the key yet:"; say "$out"; exit 1 ;;
esac
