#!/usr/bin/env bash
# Checks a translated javac against the real one (see docs/large-projects.md, "javac end to end"):
# each case is compiled by both with the same arguments, each in a copy of the case's sources of
# its own, and the class files must be the same byte for byte, as must standard output, standard
# error and the exit code.
#
#   scripts/javac-check.sh TRANSLATED_JAVAC [CASE...]            # the cases named, or all of them
#   scripts/javac-check.sh TRANSLATED_JAVAC --corpus [PATTERN]   # the corpus, or the cases matching
#
# A case of scripts/javac-check/ is a directory of .java files, compiled together with `-d classes`,
# and an optional file `args` with more of javac's arguments, split at white space, which go first.
# A case may also have a directory jar/ of .java files, which the real javac compiles and the JDK's
# jar tool packs into lib.jar next to the case's sources, for its args to put on the class path.
# The corpus cases are the programs of corpus/: each file of features, threads and whole-program on
# its own, each file of programs on its own and all of them together, and each project (a directory
# with a src/) with all its files. PATTERN is a shell pattern that case names must match, such as
# 'features-*'.
# scripts/javac-check/known-differences lists cases expected to differ, one per line: the name, then
# the reason. They are reported as KNOWN and do not fail the run. Every other differing case does.
# Prints one line per differing case and a summary; the exit status is 0 only if nothing differs.
#
# JAVAC              the real javac (default: javac on PATH); its JDK must be the one whose
#                    src.zip the translation was made from, so that the class files can match
# JAVAC_CHECK_OUT    the work directory, emptied for each case (default: ${TMPDIR:-/tmp}/javac-check)
# SLOT               a command that each run of the translated javac runs under, such as a
#                    build-slot limiter (default: none)
# TIMEOUT            seconds that a run may take (default 600)
set -euo pipefail
shopt -s nullglob

root=$(cd "$(dirname "$0")/.." && pwd)
usage() { sed -n '2,/^set -euo/p' "$0" | sed '$d' >&2; exit 2; }
[ $# -ge 1 ] || usage
translated=$(realpath "$1")
shift
[ -x "$translated" ] || { echo "no translated javac at $translated" >&2; exit 2; }
corpus=
filter='*'
if [ "${1:-}" = --corpus ]; then
  corpus=1
  shift
  [ $# -le 1 ] || usage
  filter=${1:-*}
fi
real=$(realpath "$(command -v "${JAVAC:-javac}")")
jar_tool=$(dirname "$real")/jar
# The translated javac reads the platform's classes from the JDK it is told is home.
java_home=$(dirname "$(dirname "$real")")
out=${JAVAC_CHECK_OUT:-${TMPDIR:-/tmp}/javac-check}
read -r -a slot <<< "${SLOT:-}"
timeout=${TIMEOUT:-600}
cases_dir=$root/scripts/javac-check
known_file=$cases_dir/known-differences

# Prints the case of scripts/javac-check/ named $1, as name|source root|files|javac arguments.
case_line() {
  local name=$1 dir=$cases_dir/$1 files args=
  [ -d "$dir" ] || { echo "no case $name" >&2; exit 2; }
  files=$(cd "$dir" && ls -- *.java | tr '\n' ' ')
  [ ! -f "$dir/args" ] || args=$(head -n 1 "$dir/args")
  echo "$name|scripts/javac-check/$name|$files|$args"
}

# Prints the corpus cases in the same format as case_line; the roots are relative to the repository.
corpus_lines() {
  local f dir name group files
  for f in "$root"/corpus/features/*.java "$root"/corpus/threads/*.java \
    "$root"/corpus/whole-program/*.java "$root"/corpus/programs/*.java; do
    dir=$(dirname "$f")
    echo "$(basename "$dir")-$(basename "$f" .java)|${dir#"$root"/}|$(basename "$f")|"
  done
  files=$(cd "$root/corpus/programs" && ls -- *.java | tr '\n' ' ')
  echo "programs-all|corpus/programs|$files|"
  for dir in "$root"/corpus/projects/*/ "$root"/corpus/whole-program/*/; do
    [ -d "$dir/src" ] || continue
    group=$(basename "$(dirname "$dir")")
    name=$(basename "$dir")
    files=$(cd "$dir/src" && find . -name '*.java' | sed 's|^\./||' | sort | tr '\n' ' ')
    echo "$group-$name|${dir#"$root"/}src|$files|"
  done
}

# Prints the reason that case $1 is listed with in known-differences; fails if it is not listed.
known_reason() {
  local name reason
  [ -f "$known_file" ] || return 1
  while read -r name reason; do
    if [ "$name" = "$1" ]; then echo "$reason"; return 0; fi
  done < "$known_file"
  return 1
}

# Makes $1/lib.jar from the sources of the case's jar/ directory, with the real javac.
make_jar() {
  local dir=$1 sources
  rm -rf "$dir" && mkdir -p "$dir/classes"
  mapfile -t sources < <(cd "$src_root/jar" && find . -name '*.java' | sort)
  (cd "$src_root/jar" && "$real" -d "$dir/classes" "${sources[@]}") || { echo "cannot compile $src_root/jar" >&2; exit 2; }
  "$jar_tool" --create --file "$dir/lib.jar" -C "$dir/classes" .
}

# Compiles the files src_files of the case in src_root with javac $2 in directory $1, with the
# arguments src_args first, keeping output, errors and exit code there. Words after the second
# argument run first, such as the build-slot limiter.
compile() {
  local dir=$1 javac=$2
  shift 2
  rm -rf "$dir" && mkdir -p "$dir/classes"
  (cd "$src_root" && cp --parents -- "${src_files[@]}" "$dir/")
  [ ! -d "$src_root/jar" ] || cp "$work/jar/lib.jar" "$dir/"
  (cd "$dir" && set +e && JAVA_HOME=$java_home timeout "$timeout" "$@" "$javac" "${src_args[@]}" \
    -d classes "${src_files[@]}" > stdout 2> stderr < /dev/null; echo $? > exit)
}

if [ -n "$corpus" ]; then
  mapfile -t all < <(corpus_lines)
else
  names=("$@")
  if [ ${#names[@]} -eq 0 ]; then
    for dir in "$cases_dir"/*/; do names+=("$(basename "$dir")"); done
  fi
  all=()
  for name in "${names[@]}"; do all+=("$(case_line "$name")"); done
fi

checked=0 identical=0 known=0 differ=0
for line in "${all[@]}"; do
  IFS='|' read -r name rel files args <<< "$line"
  [[ $name == $filter ]] || continue
  checked=$((checked + 1))
  src_root=$root/$rel
  read -r -a src_files <<< "$files"
  read -r -a src_args <<< "$args"
  work=$out/$name
  [ ! -d "$src_root/jar" ] || make_jar "$work/jar"
  compile "$work/real" "$real"
  compile "$work/translated" "$translated" "${slot[@]}"
  problems=()
  for part in exit stdout stderr; do
    cmp -s "$work/real/$part" "$work/translated/$part" || problems+=("$part")
  done
  diff -rq "$work/real/classes" "$work/translated/classes" > "$work/classdiff.txt" 2>&1 || true
  [ ! -s "$work/classdiff.txt" ] || problems+=("class files differ: $(wc -l < "$work/classdiff.txt")")
  if reason=$(known_reason "$name"); then listed=1; else listed=0; fi
  if [ ${#problems[@]} -eq 0 ]; then
    identical=$((identical + 1))
    if [ "$listed" -eq 1 ]; then
      echo "NOTE $name: listed in known-differences but identical; remove it from there"
    elif [ -z "$corpus" ]; then
      echo "PASS $name ($(find "$work/real/classes" -name '*.class' | wc -l) class files, exit $(cat "$work/real/exit"))"
    fi
    continue
  fi
  joined=$(IFS=', '; echo "${problems[*]}")
  if [ "$listed" -eq 1 ]; then
    known=$((known + 1))
    echo "KNOWN $name: $joined ($reason)"
  else
    differ=$((differ + 1))
    echo "DIFF $name: $joined (see $work)"
  fi
done
[ "$checked" -gt 0 ] || { echo "no case matches $filter" >&2; exit 2; }
echo "$checked cases, $identical identical, $known known, $differ differ"
[ "$differ" -eq 0 ]
