#!/usr/bin/env bash
# Translates, compiles and runs each target program and reports how far it gets:
# REJECTED (the translator refused it), DOES NOT COMPILE (rustc error), WRONG OUTPUT
# (stdout or exit code differs from Name.expected and Name.exitcode), TIMED OUT (a step took
# longer than its limit), or PASS.
# Usage: targets/run.sh [JDK_JAVA_BASE_ROOT] [NAME...]
# Without the root each file is translated alone; with it, the program is translated whole,
# reading JDK classes from that source root. Names pick programs (all of them without names).
# Scratch output goes to targets/ in the cargo target directory (CARGO_TARGET_DIR, or target/);
# each program's line is also appended to results.txt there as it finishes, so a run that is cut short leaves its record.
# A whole-program translation takes a few seconds and about 1 GB of memory (javac's), and a
# translated crate takes longer to build: start a full run in the background rather than under a
# short command timeout. The translations run TARGETS_JOBS at a time (2 by default); the builds
# and runs then go one by one, in order.
# A program whose translated crate is the same as in an earlier run that passed, with the same
# runtime crate, toolchain, input and arguments, is not built and run again: its output and exit
# code are taken from the checks' cache (CHECK_CACHE, by default ~/.cache/java-to-rust-checks;
# see "Testing" in README.md) and compared with Name.expected as a fresh run's would be.
# CHECK_FRESH=1 builds and runs every program, and stores the passing runs anew.
# Limits in seconds: TRANSLATE_TIMEOUT (600), BUILD_TIMEOUT (900), RUN_TIMEOUT (20).
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
targets="$root/targets"
work="${CARGO_TARGET_DIR:-$root/target}/targets"
library=
if [ $# -gt 0 ] && [ -d "$1" ]; then
  library=$1
  shift
fi
translate_timeout=${TRANSLATE_TIMEOUT:-600}
build_timeout=${BUILD_TIMEOUT:-900}
run_timeout=${RUN_TIMEOUT:-20}
jobs=${TARGETS_JOBS:-2}

if [ $# -gt 0 ]; then
  programs=()
  for name in "$@"; do
    [ -f "$targets/$name.java" ] || { echo "no target program $name" >&2; exit 2; }
    programs+=("$targets/$name.java")
  done
else
  programs=("$targets"/*.java)
fi

mkdir -p "$work"
cargo build -q --profile checks -p java-to-rust || exit 1
# A copy, so that rebuilding the translator during a run does not change the programs after it.
translator="$work/java-to-rust"
cp "${CARGO_TARGET_DIR:-$root/target}/checks/java-to-rust" "$translator" || exit 1
results="$work/results.txt"
: > "$results"
runtime=$(cd "$root/crates/runtime" && pwd -P)
cache=${CHECK_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/java-to-rust-checks}/targets
fresh=${CHECK_FRESH:-}

# A hash of standard input, one line.
hash() {
  sha256sum | cut -c1-32
}

# Each file under the current directory, in order, with its path and the hash of its contents,
# leaving out the directories named target.
hash_files() {
  find . -name target -prune -o -type f -print | LC_ALL=C sort | while IFS= read -r f; do
    printf '%s %s\n' "$f" "$(hash < "$f")"
  done
}

# What, besides a program's own crate, decides the binary built from it and what it prints, as in
# check_cache::rust_build_key: the runtime crate and the manifests it inherits from, the
# toolchain, cargo's configuration files, and the variables of cargo and Rust that can change a
# build or a run (not those of where and how many jobs build).
build_key=$({
  echo "targets rust build"
  (cd "$runtime" && hash_files)
  d=$runtime
  while [ "$d" != / ]; do
    d=$(dirname "$d")
    [ -f "$d/Cargo.toml" ] && hash < "$d/Cargo.toml"
  done
  (cd "$work" && "${RUSTC:-rustc}" -vV && cargo -V)
  d=$work
  while :; do
    for f in "$d/.cargo/config" "$d/.cargo/config.toml"; do [ -f "$f" ] && hash < "$f"; done
    [ "$d" = / ] && break
    d=$(dirname "$d")
  done
  for f in "${CARGO_HOME:-$HOME/.cargo}/config" "${CARGO_HOME:-$HOME/.cargo}/config.toml"; do
    [ -f "$f" ] && hash < "$f"
  done
  env | grep -E '^(CARGO|RUST)' | grep -vE '^(CARGO|CARGO_BUILD_JOBS|CARGO_TARGET_DIR|CARGO_TARGET_TMPDIR|CARGO_MANIFEST_DIR|CARGO_MANIFEST_PATH|CARGO_MAKEFLAGS|CARGO_PRIMARY_PACKAGE|CARGO_CRATE_NAME|RUST_TEST_THREADS|RUST_RECURSION_COUNT|RUST_BACKTRACE|RUST_LIB_BACKTRACE|CARGO_PKG_[A-Z_]*|CARGO_BIN_[A-Za-z0-9_-]*)=' | LC_ALL=C sort
} | hash)

# The key of a translated program's run: the build key, the crate (its manifest without the
# runtime's path, which is another in each worktree), and the program's input and arguments.
run_key() {
  local name=$1 dir=$2 manifest
  manifest=$(cat "$dir/Cargo.toml") || return 1
  {
    echo "targets rust run $build_key $name"
    printf '%s\n' "${manifest//"$runtime"/<runtime>}" | hash
    (cd "$dir" && find src -type f | LC_ALL=C sort | while IFS= read -r f; do printf '%s %s\n' "$f" "$(hash < "$f")"; done)
    echo input
    [ -f "$targets/$name.input" ] && hash < "$targets/$name.input"
    echo arguments
    [ -f "$targets/$name.args" ] && hash < "$targets/$name.args"
  } | hash
}

# Translates one program into its directory, leaving the translator's output in translate.out
# and then its exit code in translate.code.
translate() {
  local java=$1 name dir
  name=$(basename "$java" .java)
  dir="$work/$name"
  mkdir -p "$dir"
  if [ -n "$library" ]; then
    # A listed file next to a module's source root is rejected by javac ("file should be on
    # source path"), so the program goes in as a source directory of its own.
    mkdir -p "$dir/src-java"
    cp "$java" "$dir/src-java/"
    timeout "$translate_timeout" "$translator" --main "$name" --source "$dir/src-java" --library "$library" --crate "$dir" > "$dir/translate.out" 2>&1
  else
    timeout "$translate_timeout" "$translator" "$java" --crate "$dir" > "$dir/translate.out" 2>&1
  fi
  echo $? > "$dir/translate.tmp"
  mv "$dir/translate.tmp" "$dir/translate.code"
}

# The translations run in the background, while the loop below builds and runs each program as
# soon as its translation is done.
for java in "${programs[@]}"; do
  rm -rf "$work/$(basename "$java" .java)"
done
(
  running=0
  for java in "${programs[@]}"; do
    translate "$java" &
    running=$((running + 1))
    if [ $running -ge "$jobs" ]; then
      wait -n
      running=$((running - 1))
    fi
  done
  wait
) &
translations=$!
trap 'kill $translations 2>/dev/null' EXIT
stored=0

passed=0
rejected=0
nocompile=0
wrong=0
timedout=0
total=0

report() {
  printf '%-24s %-16s %s\n' "$1" "$2" "$3" | tee -a "$results"
}

for java in "${programs[@]}"; do
  name=$(basename "$java" .java)
  crate=$(printf '%s' "$name" | tr 'A-Z' 'a-z' | tr -c 'a-z0-9' '-')
  dir="$work/$name"
  bin="$work/cargo/debug/$crate"
  total=$((total + 1))
  while [ ! -f "$dir/translate.code" ] && kill -0 $translations 2>/dev/null; do
    sleep 0.2
  done
  translated=$(cat "$dir/translate.out" 2>/dev/null || echo "the translation did not finish")
  code=$(cat "$dir/translate.code" 2>/dev/null || echo 1)
  if [ $code -eq 124 ]; then
    timedout=$((timedout + 1))
    report "$name" "TIMED OUT" "translation took more than ${translate_timeout}s"
    continue
  elif [ $code -ne 0 ]; then
    rejected=$((rejected + 1))
    # The translator's message, or how it died (a signal: 137 is the kernel's out-of-memory kill).
    message=$(printf '%s\n' "$translated" | grep -v '^\s*$' | head -n 1)
    [ $code -gt 128 ] && message="translator killed by signal $((code - 128)): $message"
    report "$name" "REJECTED" "$message"
    continue
  fi

  input=/dev/null
  [ -f "$targets/$name.input" ] && input="$targets/$name.input"
  # The command-line arguments, one per line of Name.args, as the program's own main gets them.
  args=()
  [ -f "$targets/$name.args" ] && mapfile -t args < "$targets/$name.args"
  expected_code=0
  [ -f "$targets/$name.exitcode" ] && expected_code=$(tr -d '\n' < "$targets/$name.exitcode")
  actual="$dir/actual.out"
  key=$(run_key "$name" "$dir")
  entry="$cache/${key:0:2}/$key"
  if [ -n "$key" ] && [ "$fresh" != 1 ] && [ -f "$entry" ]; then
    # The stored run: its exit code on the first line, then its standard output.
    code=$(head -n 1 "$entry")
    tail -n +2 "$entry" > "$actual"
    stored=$((stored + 1))
    key=
  else
    rm -f "$bin"
    built=$(CARGO_TARGET_DIR="$work/cargo" timeout "$build_timeout" cargo build -q --manifest-path "$dir/Cargo.toml" 2>&1)
    if [ $? -eq 124 ]; then
      timedout=$((timedout + 1))
      report "$name" "TIMED OUT" "cargo build took more than ${build_timeout}s"
      continue
    fi
    if [ ! -x "$bin" ]; then
      nocompile=$((nocompile + 1))
      report "$name" "DOES NOT COMPILE" "$(printf '%s\n' "$built" | grep '^error' | grep -v 'could not compile' | head -n 1)"
      continue
    fi
    timeout "$run_timeout" "$bin" "${args[@]}" < "$input" > "$actual" 2> "$dir/stderr.txt"
    code=$?
  fi

  if [ "$code" = "$expected_code" ] && cmp -s "$actual" "$targets/$name.expected"; then
    passed=$((passed + 1))
    report "$name" "PASS" ""
    if [ -n "$key" ]; then
      mkdir -p "$(dirname "$entry")"
      tmp=$(mktemp "$entry.XXXXXX") && { echo "$code"; cat "$actual"; } > "$tmp" && mv "$tmp" "$entry"
    fi
  else
    wrong=$((wrong + 1))
    detail="exit $code, expected $expected_code"
    [ "$code" = 124 ] && detail="ran more than ${run_timeout}s"
    if ! cmp -s "$actual" "$targets/$name.expected"; then
      detail="$detail; stdout differs: $(diff "$targets/$name.expected" "$actual" | head -n 1)"
    fi
    report "$name" "WRONG OUTPUT" "$detail"
  fi
done

echo | tee -a "$results"
echo "$stored of $total programs were not built and run: their crates were the same as in a run that passed" | tee -a "$results"
echo "summary: $passed PASS, $wrong WRONG OUTPUT, $nocompile DOES NOT COMPILE, $rejected REJECTED, $timedout TIMED OUT, $total programs" | tee -a "$results"
