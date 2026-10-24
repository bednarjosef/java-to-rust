#!/usr/bin/env bash
# Builds a translated crate in release with profile-guided optimization (see docs/performance.md,
# "The translated javac"): a build that records a profile when it runs, a training run of it, and
# a build that uses the merged profile.
#
#   scripts/pgo-build.sh CRATE_DIR TRAIN_COMMAND [ARG...]
#
# TRAIN_COMMAND runs with the instrumented binary as its first argument, before ARG...; it should
# run the program on typical inputs, which should not be the ones it is then timed on. Prints the
# path of the optimized binary. The crate's release profile applies to both builds (the
# translator writes fat LTO there), and so do the flags of its .cargo/config.toml, to which the
# profile flags are added. Needs llvm-profdata, from `rustup component add llvm-tools`.
#
# CARGO_TARGET_DIR   where both builds go, as pgo-generate/ and pgo-use/ (default: CRATE_DIR/target)
set -euo pipefail

[ $# -ge 2 ] || { sed -n '2,/^set -euo/p' "$0" | sed '$d' >&2; exit 2; }
crate=$(realpath "$1")
shift
target=${CARGO_TARGET_DIR:-$crate/target}
mkdir -p "$target"
target=$(realpath "$target")
profdata=$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/llvm-profdata
[ -x "$profdata" ] || { echo "no llvm-profdata: run rustup component add llvm-tools" >&2; exit 2; }
name=$(sed -n 's/^name = "\(.*\)"/\1/p' "$crate/Cargo.toml" | head -n 1)
data=$target/pgo-data

rm -rf "$data"
mkdir -p "$data"
(cd "$crate" && CARGO_TARGET_DIR=$target/pgo-generate CARGO_BUILD_RUSTFLAGS="-Cprofile-generate=$data" \
  cargo build --release -q)
train=$1
shift
"$train" "$target/pgo-generate/release/$name" "$@" >&2
"$profdata" merge -o "$target/merged.profdata" "$data"
(cd "$crate" && CARGO_TARGET_DIR=$target/pgo-use CARGO_BUILD_RUSTFLAGS="-Cprofile-use=$target/merged.profdata" \
  cargo build --release -q)
echo "$target/pgo-use/release/$name"
