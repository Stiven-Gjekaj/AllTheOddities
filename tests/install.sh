#!/bin/sh
# Tests install.sh against a fake release on a local web server.
# It does not touch the real home directory.
#
# Usage: sh tests/install.sh

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
server=""
cleanup() { [ -n "$server" ] && kill "$server" 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT INT TERM

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
sum() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# A fake odd for each target of macOS and Linux. It prints its arguments.
release="$work/release"
mkdir -p "$release"
for t in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
    printf '#!/bin/sh\necho "fake odd: $*"\n' > "$release/odd-$t"
    echo "$(sum "$release/odd-$t")  odd-$t" >> "$release/SHA256SUMS"
done

port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$release" >/dev/null 2>&1 &
server=$!
i=0
until curl -fsS "http://127.0.0.1:$port/SHA256SUMS" >/dev/null 2>&1; do
    i=$((i + 1)); [ $i -lt 50 ] || fail "the web server did not start"; sleep 0.1
done
export ODD_BASE_URL="http://127.0.0.1:$port"

# installs_and_adds_to_path
home="$work/home1"; mkdir -p "$home"
out=$(HOME="$home" sh "$root/install.sh" 2>&1) || fail "install exited non-zero: $out"
[ -x "$home/.local/bin/odd" ] || fail "no odd at ~/.local/bin/odd"
echo "$out" | grep -q "fake odd: path add $home/.local/bin --yes" || fail "no odd path add: $out"

# arguments_run_once_and_leave_nothing (stdin is a pipe, as with curl | sh)
home="$work/home2"; mkdir -p "$home"
out=$(cat "$root/install.sh" | HOME="$home" sh -s -- ssh --out 2>&1) || fail "run once exited non-zero: $out"
echo "$out" | grep -q "fake odd: ssh --out" || fail "odd did not run with the arguments: $out"
[ ! -e "$home/.local/bin/odd" ] || fail "run once installed odd"

# a_bad_checksum_installs_nothing
# Change the first digit to another digit. A fixed 0 fails one time in 16,
# when the real checksum starts with 0.
awk '{ c = substr($0, 1, 1); print (c == "0" ? "1" : "0") substr($0, 2) }' "$release/SHA256SUMS" > "$work/bad" && mv "$work/bad" "$release/SHA256SUMS"
home="$work/home3"; mkdir -p "$home"
if out=$(HOME="$home" sh "$root/install.sh" 2>&1); then fail "a bad checksum exited 0"; fi
echo "$out" | grep -q "does not match SHA256SUMS" || fail "no checksum message: $out"
[ ! -e "$home/.local/bin/odd" ] || fail "a bad checksum installed odd"

echo PASS
