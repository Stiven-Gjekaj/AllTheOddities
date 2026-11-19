#!/bin/sh
# Runs the commands of odd that change nothing, on the host of the CI.
# No unit test reaches the real host, so this is the check that odd starts,
# reads the host and ends with exit code 0 there.
#
# Usage: sh scripts/smoke.sh <path of odd>

set -eu
odd=$1

run() {
    echo "== odd $*"
    "$odd" "$@" || { echo "FAIL: odd $* exited $?" >&2; exit 1; }
}

run --version
run list
run list --json
run path list
run path add "$(pwd)/smoke-dir" --dry-run --yes
run ssh --out --dry-run --yes
echo "== odd update --dry-run (no release yet is not a failure)"
"$odd" update --dry-run || true
echo PASS
