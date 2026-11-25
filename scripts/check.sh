#!/usr/bin/env bash
# The full local check, in order: formatting, clippy with warnings as errors, all tests; with
# JAVA_BASE_SOURCES set, also that the translator's Java holds no copy of the JDK's.
# Prints only what fails. On success with a clean working tree, records the commit in
# target/check-passed (the worker Stop hook reads it).
# Results that cannot change until their inputs do (Java's output of each test program, javac's
# trees, the runs of unchanged translations) are kept in CHECK_CACHE, by default
# ~/.cache/java-to-rust-checks, shared by every worktree; CHECK_FRESH=1 does all the work again
# and stores the results anew (see docs/testing.md).
set -uo pipefail
cd "$(dirname "$0")/.."
export CHECK_CACHE=${CHECK_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/java-to-rust-checks}

fail() { echo "check failed: $1"; exit 1; }

markers=$(git grep -nE '^(<<<<<<<|>>>>>>>) ' -- . ':!scripts/check.sh')
[ -z "$markers" ] || { echo "$markers" | head -20; fail "merge conflict markers"; }
# With the JDK's sources: no file of the translator's Java copies the JDK's (docs/jdk-overlays.md).
if [ -n "${JAVA_BASE_SOURCES:-}" ]; then
    out=$(python3 -I scripts/check-jdk-copies.py "$JAVA_BASE_SOURCES" 2>&1) || { echo "$out" | head -40; fail "JDK code in crates/translator/java (see docs/jdk-overlays.md)"; }
fi
cargo fmt --check >/dev/null 2>&1 || { cargo fmt --check 2>&1 | head -40; fail "cargo fmt (run \`cargo fmt\`)"; }
out=$(cargo clippy --all-targets -q -- -D warnings 2>&1) || { echo "$out" | head -80; fail "clippy"; }
# The corpus test lists every program; only those that do not pass are shown.
# The integration tests and doctests build in the checks profile, the dev profile optimized
# (Cargo.toml): they spend their time translating, which runs several times faster optimized, and
# targets/run.sh uses the same build of the translator. The unit tests, a second build of each
# crate that runs in seconds, build in the dev profile, which compiles faster.
tests() {
    cargo test -q --lib && cargo test -q --profile checks --bins --test '*' && cargo test -q --profile checks --doc
}
out=$(tests 2>&1) || {
    echo "$out" | grep -vE '^\s*$|^running |^test result: ok| PASS$| ok( \([0-9]+ rustc warnings\))?$|^[.]+( [0-9]+/[0-9]+)?$|: warning: ' | head -120
    fail "tests"
}

mkdir -p target
if [ -z "$(git status --porcelain)" ]; then
    git rev-parse HEAD > target/check-passed
    echo "check passed on $(git rev-parse --short HEAD)"
else
    rm -f target/check-passed
    echo "check passed, but the working tree has uncommitted changes: commit, then run it again"
fi
