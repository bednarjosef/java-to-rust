#!/usr/bin/env bash
# Everything CI checks, in its order: formatting, clippy with warnings as errors, all tests.
# Prints only what fails. On success with a clean working tree, records the commit in
# target/check-passed (the worker Stop hook reads it).
set -uo pipefail
cd "$(dirname "$0")/.."

fail() { echo "check failed: $1"; exit 1; }

cargo fmt --check >/dev/null 2>&1 || { cargo fmt --check 2>&1 | head -40; fail "cargo fmt (run \`cargo fmt\`)"; }
out=$(cargo clippy --all-targets -q -- -D warnings 2>&1) || { echo "$out" | head -80; fail "clippy"; }
out=$(cargo test -q 2>&1) || { echo "$out" | grep -vE '^\s*$|^running |^test result: ok' | head -120; fail "tests"; }

mkdir -p target
if [ -z "$(git status --porcelain)" ]; then
    git rev-parse HEAD > target/check-passed
    echo "check passed on $(git rev-parse --short HEAD)"
else
    rm -f target/check-passed
    echo "check passed, but the working tree has uncommitted changes: commit, then run it again"
fi
