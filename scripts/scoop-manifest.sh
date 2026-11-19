#!/usr/bin/env bash
# Write the Scoop manifest of one release, from the SHA256SUMS of that
# release.
#
# The manifest installs the Windows binary of the release, for x86_64 and for
# arm64. A person puts it into the bucket and commits it there: no bot pushes
# to the bucket.
#
# Usage: scripts/scoop-manifest.sh <version> <SHA256SUMS> [base-url] > alltheoddities.json

set -euo pipefail

USAGE="usage: scoop-manifest.sh <version> <SHA256SUMS> [base-url]"
VERSION="${1:?$USAGE}"
SUMS="${2:?$USAGE}"
BASE="${3:-https://github.com/Stiven-Gjekaj/AllTheOddities/releases/download/v${VERSION}}"
SCRIPT="scoop-manifest.sh"
source "$(dirname "$0")/lib-digest.sh"

X64="$(digest odd-x86_64-pc-windows-msvc.exe)"
ARM64="$(digest odd-aarch64-pc-windows-msvc.exe)"

# The address of a later release, for the autoupdate of Scoop. Scoop puts the
# version where $version stands, and it hashes the file itself.
LATER='https://github.com/Stiven-Gjekaj/AllTheOddities/releases/download/v$version/odd'

cat <<MANIFEST
{
    "version": "${VERSION}",
    "description": "${DESCRIPTION}",
    "homepage": "https://github.com/Stiven-Gjekaj/AllTheOddities",
    "license": "MIT",
    "notes": "Scoop updates odd. Use scoop update alltheoddities, and not odd update. Some oddities need a shell that runs as Administrator.",
    "architecture": {
        "64bit": {
            "url": "${BASE}/odd-x86_64-pc-windows-msvc.exe#/odd.exe",
            "hash": "${X64}"
        },
        "arm64": {
            "url": "${BASE}/odd-aarch64-pc-windows-msvc.exe#/odd.exe",
            "hash": "${ARM64}"
        }
    },
    "bin": "odd.exe",
    "checkver": "github",
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "${LATER}-x86_64-pc-windows-msvc.exe#/odd.exe"
            },
            "arm64": {
                "url": "${LATER}-aarch64-pc-windows-msvc.exe#/odd.exe"
            }
        }
    }
}
MANIFEST
