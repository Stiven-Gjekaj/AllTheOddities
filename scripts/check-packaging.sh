#!/usr/bin/env bash
# Run each package script on a SHA256SUMS that this script makes, and check
# the shape of what each one writes. The digests are made up, and nothing is
# downloaded or published.
#
# Usage: scripts/check-packaging.sh

set -euo pipefail

cd "$(dirname "$0")/.."
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

version="9.8.7"
for file in \
    odd-x86_64-unknown-linux-musl \
    odd-aarch64-unknown-linux-musl \
    odd-x86_64-apple-darwin \
    odd-aarch64-apple-darwin \
    odd-x86_64-pc-windows-msvc.exe \
    odd-aarch64-pc-windows-msvc.exe; do
    printf '%064x  %s\n' "${#file}" "$file" >> "$work/SHA256SUMS"
done

scripts/homebrew-formula.sh "$version" "$work/SHA256SUMS" > "$work/alltheoddities.rb"
grep -q 'class Alltheoddities < Formula' "$work/alltheoddities.rb"
test "$(grep -c 'sha256 "' "$work/alltheoddities.rb")" = 4
grep -q "releases/download/v${version}/odd-aarch64-apple-darwin" "$work/alltheoddities.rb"
if command -v ruby >/dev/null 2>&1; then
    ruby -c "$work/alltheoddities.rb" > /dev/null
fi

scripts/scoop-manifest.sh "$version" "$work/SHA256SUMS" > "$work/alltheoddities.json"
python3 -c 'import json, sys; m = json.load(open(sys.argv[1])); assert m["version"] == sys.argv[2]; assert m["bin"] == "odd.exe"; assert "v" + sys.argv[2] in m["architecture"]["64bit"]["url"]' \
    "$work/alltheoddities.json" "$version"

# A SHA256SUMS that lacks a file stops each script, with the name of the file.
grep -v 'apple-darwin' "$work/SHA256SUMS" > "$work/SHORT"
if scripts/homebrew-formula.sh "$version" "$work/SHORT" > /dev/null 2> "$work/error"; then
    echo "homebrew-formula.sh took a SHA256SUMS with no macOS binary" >&2
    exit 1
fi
grep -q "apple-darwin" "$work/error"
grep -v 'windows' "$work/SHA256SUMS" > "$work/SHORT"
if scripts/scoop-manifest.sh "$version" "$work/SHORT" > /dev/null 2> "$work/error"; then
    echo "scoop-manifest.sh took a SHA256SUMS with no Windows binary" >&2
    exit 1
fi
grep -q "windows" "$work/error"

echo "Each package script wrote a file of the right shape."
