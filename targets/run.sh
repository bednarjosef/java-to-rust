#!/usr/bin/env bash
# Translates, compiles and runs each target program and reports how far it gets:
# REJECTED (the translator refused it), DOES NOT COMPILE (rustc error), WRONG OUTPUT
# (stdout or exit code differs from Name.expected and Name.exitcode), TIMED OUT (a step took
# longer than its limit), or PASS.
# Usage: targets/run.sh [JDK_JAVA_BASE_ROOT] [NAME...]
# Without the root each file is translated alone; with it, the program is translated whole,
# reading JDK classes from that source root. Names pick programs (all of them without names).
# Scratch output goes to target/targets/; each program's line is also appended to
# target/targets/results.txt as it finishes, so a run that is cut short leaves its record.
# A whole-program translation takes a few seconds and about 1 GB of memory (javac's), and a
# translated crate takes longer to build: start a full run in the background rather than under a
# short command timeout.
# Limits in seconds: TRANSLATE_TIMEOUT (600), BUILD_TIMEOUT (900), RUN_TIMEOUT (20).
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
targets="$root/targets"
work="$root/target/targets"
library=
if [ $# -gt 0 ] && [ -d "$1" ]; then
  library=$1
  shift
fi
translate_timeout=${TRANSLATE_TIMEOUT:-600}
build_timeout=${BUILD_TIMEOUT:-900}
run_timeout=${RUN_TIMEOUT:-20}

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
cargo build -q -p java-to-rust || exit 1
# A copy, so that rebuilding the translator during a run does not change the programs after it.
translator="$work/java-to-rust"
cp "${CARGO_TARGET_DIR:-$root/target}/debug/java-to-rust" "$translator" || exit 1
results="$work/results.txt"
: > "$results"

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
  rm -rf "$dir"
  mkdir -p "$dir"

  if [ -n "$library" ]; then
    # A listed file next to a module's source root is rejected by javac ("file should be on
    # source path"), so the program goes in as a source directory of its own.
    mkdir -p "$dir/src-java"
    cp "$java" "$dir/src-java/"
    translated=$(timeout "$translate_timeout" "$translator" --main "$name" --source "$dir/src-java" --library "$library" --crate "$dir" 2>&1)
  else
    translated=$(timeout "$translate_timeout" "$translator" "$java" --crate "$dir" 2>&1)
  fi
  code=$?
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

  input=/dev/null
  [ -f "$targets/$name.input" ] && input="$targets/$name.input"
  # The command-line arguments, one per line of Name.args, as the program's own main gets them.
  args=()
  [ -f "$targets/$name.args" ] && mapfile -t args < "$targets/$name.args"
  expected_code=0
  [ -f "$targets/$name.exitcode" ] && expected_code=$(tr -d '\n' < "$targets/$name.exitcode")
  actual="$dir/actual.out"
  timeout "$run_timeout" "$bin" "${args[@]}" < "$input" > "$actual" 2> "$dir/stderr.txt"
  code=$?

  if [ "$code" = "$expected_code" ] && cmp -s "$actual" "$targets/$name.expected"; then
    passed=$((passed + 1))
    report "$name" "PASS" ""
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
echo "summary: $passed PASS, $wrong WRONG OUTPUT, $nocompile DOES NOT COMPILE, $rejected REJECTED, $timedout TIMED OUT, $total programs" | tee -a "$results"
