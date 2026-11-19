#!/bin/sh
# Tests new-machine.sh in offline mode, in a home directory that the test makes.
# It does not touch the real ~/.ssh.

set -eu

script="$(cd "$(dirname "$0")" && pwd)/new-machine.sh"
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

# The config has an entry before the run. The script must keep it.
mkdir -p "$home/.ssh"
printf 'Host old\n    HostName old.example\n' >"$home/.ssh/config"

HOME="$home" sh "$script" -N -o -c test@test >/dev/null

[ -f "$home/.ssh/id_ed25519" ] || fail "no private key"
[ -f "$home/.ssh/id_ed25519.pub" ] || fail "no public key"
grep -q 'test@test' "$home/.ssh/id_ed25519.pub" || fail "the comment is not in the key"

# GNU stat takes -c. BSD stat takes -f, and GNU stat reads -f as "file system",
# so try GNU first.
perm() { stat -c %a "$1" 2>/dev/null || stat -f %Lp "$1"; }
[ "$(perm "$home/.ssh")" = 700 ] || fail "~/.ssh is not 700"
[ "$(perm "$home/.ssh/id_ed25519")" = 600 ] || fail "the key is not 600"
[ "$(perm "$home/.ssh/config")" = 600 ] || fail "the config is not 600"

grep -q 'Host old' "$home/.ssh/config" || fail "the old entry is lost"
grep -qE '^Host[[:space:]]+github\.com$' "$home/.ssh/config" || fail "no GitHub block"

# A second run keeps the key and does not add a second block.
before=$(cat "$home/.ssh/id_ed25519.pub")
HOME="$home" sh "$script" -N -o >/dev/null
[ "$before" = "$(cat "$home/.ssh/id_ed25519.pub")" ] || fail "the second run changed the key"
n=$(grep -cE '^Host[[:space:]]+github\.com$' "$home/.ssh/config")
[ "$n" = 1 ] || fail "the second run added a block ($n blocks)"

# SSH must read the config without an error.
HOME="$home" ssh -G -F "$home/.ssh/config" github.com | grep -q "identityfile $home/.ssh/id_ed25519" \
    || fail "ssh does not read the key path from the config"

echo "PASS"
