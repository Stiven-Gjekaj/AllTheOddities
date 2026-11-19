#!/bin/sh
# Installs odd, the command of AllTheOddities, on macOS or Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.sh | sh
#
# With arguments, it runs odd one time and installs nothing:
#
#   curl -fsSL https://raw.githubusercontent.com/Stiven-Gjekaj/AllTheOddities/main/install.sh | sh -s -- ssh
#
# ODD_VERSION=v2026.12.0 selects a release. The default is the latest one.
# The script checks the binary against SHA256SUMS before it runs it.

set -eu

repo=https://github.com/Stiven-Gjekaj/AllTheOddities
if [ -n "${ODD_BASE_URL:-}" ]; then
    base=$ODD_BASE_URL # for the tests only
elif [ -n "${ODD_VERSION:-}" ]; then
    base=$repo/releases/download/$ODD_VERSION
else
    base=$repo/releases/latest/download
fi

case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-musl ;;
    *) echo "odd: this installer is for macOS and Linux. On Windows, use install.ps1." >&2; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) echo "odd: no build for the processor $(uname -m)." >&2; exit 1 ;;
esac
file=odd-$arch-$os

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        echo "odd: this host has no curl and no wget." >&2
        exit 1
    fi
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

fetch "$base/$file" "$tmp/odd"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"
want=$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
if [ -z "$want" ] || [ "$want" != "$(sha256 "$tmp/odd")" ]; then
    echo "The checksum of $file does not match SHA256SUMS. Nothing is installed." >&2
    exit 1
fi
chmod +x "$tmp/odd"

if [ $# -gt 0 ]; then
    # Run one time. With curl | sh, the standard input is the pipe of curl,
    # so give odd the terminal, where it can ask its questions.
    status=0
    if (: </dev/tty) 2>/dev/null; then
        "$tmp/odd" "$@" </dev/tty || status=$?
    else
        "$tmp/odd" "$@" || status=$?
    fi
    exit "$status"
fi

dir=$HOME/.local/bin
mkdir -p "$dir"
mv "$tmp/odd" "$dir/odd"
echo "odd is in $dir/odd."
"$dir/odd" path add "$dir" --yes || echo "Add $dir to PATH to run odd from any directory." >&2
"$dir/odd" --version
