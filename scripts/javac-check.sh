#!/usr/bin/env bash
# Checks a translated javac against the real one (see docs/large-projects.md, "javac end to end"):
# each case of scripts/javac-check/ is compiled by both with the same arguments, each in a copy of
# the case's directory of its own, and the class files must be the same byte for byte, as must
# standard output, standard error and the exit code.
#
#   scripts/javac-check.sh TRANSLATED_JAVAC [CASE...]     # all cases, or the ones named
#
# A case is a directory of .java files, compiled together with `-d classes`, and an optional file
# `args` with more of javac's arguments, split at white space, which go first.
#
# JAVAC              the real javac (default: javac on PATH); its JDK must be the one whose
#                    src.zip the translation was made from, so that the class files can match
# JAVAC_CHECK_OUT    the work directory, emptied for each case (default: ${TMPDIR:-/tmp}/javac-check)
# SLOT               a command that each run of the translated javac runs under, such as a
#                    build-slot limiter (default: none)
# TIMEOUT            seconds that a run may take (default 600)
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
[ $# -ge 1 ] || { sed -n '2,10p' "$0" >&2; exit 2; }
translated=$(realpath "$1")
shift
[ -x "$translated" ] || { echo "no translated javac at $translated" >&2; exit 2; }
real=$(realpath "$(command -v "${JAVAC:-javac}")")
# The translated javac reads the platform's classes from the JDK it is told is home.
java_home=$(dirname "$(dirname "$real")")
out=${JAVAC_CHECK_OUT:-${TMPDIR:-/tmp}/javac-check}
read -r -a slot <<< "${SLOT:-}"
timeout=${TIMEOUT:-600}
cases_dir=$root/scripts/javac-check

if [ $# -gt 0 ]; then cases=("$@"); else mapfile -t cases < <(ls "$cases_dir"); fi

# Compiles case $1 with javac $2 in directory $3, keeping its output, errors and exit code there.
compile() {
  local case=$1 javac=$2 dir=$3
  shift 3
  local args=() sources=()
  rm -rf "$dir" && mkdir -p "$dir/classes"
  cp "$cases_dir/$case"/*.java "$dir/"
  if [ -f "$cases_dir/$case/args" ]; then read -r -a args < "$cases_dir/$case/args"; fi
  (cd "$dir" && mapfile -t sources < <(ls -- *.java) \
    && set +e && JAVA_HOME=$java_home timeout "$timeout" "$@" "$javac" "${args[@]}" -d classes "${sources[@]}" \
      > stdout 2> stderr; echo $? > exit)
}

failed=0
for case in "${cases[@]}"; do
  [ -d "$cases_dir/$case" ] || { echo "no case $case" >&2; exit 2; }
  work=$out/$case
  compile "$case" "$real" "$work/real"
  compile "$case" "$translated" "$work/translated" "${slot[@]}"
  problems=()
  for part in exit stdout stderr; do
    cmp -s "$work/real/$part" "$work/translated/$part" || problems+=("$part differs")
  done
  differing=$(diff -rq "$work/real/classes" "$work/translated/classes" || true)
  [ -z "$differing" ] || problems+=("class files differ: $(echo "$differing" | wc -l)")
  if [ ${#problems[@]} -eq 0 ]; then
    echo "PASS $case ($(find "$work/real/classes" -name '*.class' | wc -l) class files, exit $(cat "$work/real/exit"))"
  else
    failed=$((failed + 1))
    echo "FAIL $case: $(IFS=';'; echo "${problems[*]}") (see $work)"
    [ -z "$differing" ] || echo "$differing" | head -5 | sed 's/^/    /'
  fi
done
echo "${#cases[@]} cases, $failed failed"
[ "$failed" -eq 0 ]
