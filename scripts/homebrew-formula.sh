#!/usr/bin/env bash
# Write the Homebrew formula of one release, from the SHA256SUMS of that
# release.
#
# The formula installs the binary of the release, for macOS on each chip and
# for Linux on each architecture. The files of a release have no version in
# their names, so the URL holds the version in the tag. A person commits the
# formula in the tap: no bot pushes to the tap.
#
# Usage: scripts/homebrew-formula.sh <version> <SHA256SUMS> [base-url] > alltheoddities.rb

set -euo pipefail

USAGE="usage: homebrew-formula.sh <version> <SHA256SUMS> [base-url]"
VERSION="${1:?$USAGE}"
SUMS="${2:?$USAGE}"
BASE="${3:-https://github.com/Stiven-Gjekaj/AllTheOddities/releases/download/v${VERSION}}"
SCRIPT="homebrew-formula.sh"
source "$(dirname "$0")/lib-digest.sh"

MAC_ARM="$(digest odd-aarch64-apple-darwin)"
MAC_INTEL="$(digest odd-x86_64-apple-darwin)"
LINUX_ARM="$(digest odd-aarch64-unknown-linux-musl)"
LINUX_INTEL="$(digest odd-x86_64-unknown-linux-musl)"

cat <<FORMULA
# scripts/homebrew-formula.sh in the AllTheOddities repository writes this
# file from the SHA256SUMS of the release. Change the script, and not this file.
class Alltheoddities < Formula
  desc "${DESCRIPTION}"
  homepage "https://github.com/Stiven-Gjekaj/AllTheOddities"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_arm do
      url "${BASE}/odd-aarch64-apple-darwin"
      sha256 "${MAC_ARM}"
    end
    on_intel do
      url "${BASE}/odd-x86_64-apple-darwin"
      sha256 "${MAC_INTEL}"
    end
  end

  on_linux do
    on_arm do
      url "${BASE}/odd-aarch64-unknown-linux-musl"
      sha256 "${LINUX_ARM}"
    end
    on_intel do
      url "${BASE}/odd-x86_64-unknown-linux-musl"
      sha256 "${LINUX_INTEL}"
    end
  end

  def install
    binary = Dir["odd-*"].first
    chmod 0755, binary
    bin.install binary => "odd"
  end

  def caveats
    <<~EOS
      Homebrew updates odd. Use brew upgrade alltheoddities, and not odd update.
    EOS
  end

  test do
    assert_match "odd #{version}", shell_output("#{bin}/odd --version")
  end
end
FORMULA
