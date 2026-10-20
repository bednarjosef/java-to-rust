#!/usr/bin/env bash
# Translates, compiles and runs each target program and reports how far it gets:
# REJECTED (the translator refused it), DOES NOT COMPILE (rustc error), WRONG OUTPUT
# (stdout or exit code differs from Name.expected and Name.exitcode), or PASS.
# Usage: targets/run.sh [JDK_JAVA_BASE_ROOT]
# Without the argument each file is translated alone; with it, the program is translated whole,
# reading JDK classes from that source root. Scratch output goes to target/targets/.
set -u

root=$(cd "$(dirname "$0")/.." && pwd)
targets="$root/targets"
work="$root/target/targets"
translator="$root/target/debug/java-to-rust"
library=${1:-}

mkdir -p "$work"
cargo build -q -p java-to-rust || exit 1

passed=0
rejected=0
nocompile=0
wrong=0
total=0

report() {
  printf '%-24s %-16s %s\n' "$1" "$2" "$3"
}

for java in "$targets"/*.java; do
  name=$(basename "$java" .java)
  crate=$(printf '%s' "$name" | tr 'A-Z' 'a-z' | tr -c 'a-z0-9' '-')
  dir="$work/$name"
  bin="$work/cargo/debug/$crate"
  total=$((total + 1))
  rm -rf "$dir"
  mkdir -p "$dir"

  if [ -n "$library" ]; then
    translated=$("$translator" --main "$name" --library "$library" "$java" --crate "$dir" 2>&1)
  else
    translated=$("$translator" "$java" --crate "$dir" 2>&1)
  fi
  if [ $? -ne 0 ]; then
    rejected=$((rejected + 1))
    report "$name" "REJECTED" "$(printf '%s\n' "$translated" | head -n 1)"
    continue
  fi

  rm -f "$bin"
  built=$(CARGO_TARGET_DIR="$work/cargo" cargo build -q --manifest-path "$dir/Cargo.toml" 2>&1)
  if [ ! -x "$bin" ]; then
    nocompile=$((nocompile + 1))
    report "$name" "DOES NOT COMPILE" "$(printf '%s\n' "$built" | grep '^error' | grep -v 'could not compile' | head -n 1)"
    continue
  fi

  input=/dev/null
  [ -f "$targets/$name.input" ] && input="$targets/$name.input"
  expected_code=0
  [ -f "$targets/$name.exitcode" ] && expected_code=$(tr -d '\n' < "$targets/$name.exitcode")
  actual="$dir/actual.out"
  timeout 20 "$bin" < "$input" > "$actual" 2> "$dir/stderr.txt"
  code=$?

  if [ "$code" = "$expected_code" ] && cmp -s "$actual" "$targets/$name.expected"; then
    passed=$((passed + 1))
    report "$name" "PASS" ""
  else
    wrong=$((wrong + 1))
    detail="exit $code, expected $expected_code"
    if ! cmp -s "$actual" "$targets/$name.expected"; then
      detail="$detail; stdout differs: $(diff "$targets/$name.expected" "$actual" | head -n 1)"
    fi
    report "$name" "WRONG OUTPUT" "$detail"
  fi
done

echo
echo "summary: $passed PASS, $wrong WRONG OUTPUT, $nocompile DOES NOT COMPILE, $rejected REJECTED, $total programs"
