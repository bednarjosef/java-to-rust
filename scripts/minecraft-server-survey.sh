#!/usr/bin/env bash
# Prepares the Minecraft: Java Edition dedicated server as a translation target and surveys how
# far the translator gets on it (see docs/minecraft-server.md).
#
#   scripts/minecraft-server-survey.sh [STEP...]      # all steps, or the ones named
#
# Steps: extract, decompile, best-of, libraries, library-sources, jdk-sources, parse, census, summary.
#
# Everything Mojang's and everything derived from it (the jars, the decompiled sources, the census
# output) stays in MINECRAFT_CACHE, outside the repository; never commit any of it.
#
# MINECRAFT_CACHE    the work directory (default ~/.cache/java-to-rust-targets/minecraft-26.3)
# MINECRAFT_JAR      the server jar from Mojang (default $MINECRAFT_CACHE/server-26.3.jar)
# VINEFLOWER         the Vineflower 1.12.0 decompiler jar (default $MINECRAFT_CACHE/vineflower-1.12.0.jar;
#                    fetched from Maven Central, org.vineflower:vineflower:1.12.0, if missing; SHA-1 below)
# CFR                the CFR 0.152 decompiler jar (default $MINECRAFT_CACHE/cfr-0.152.jar; fetched from
#                    Maven Central, org.benf:cfr:0.152, if missing; SHA-1 below)
# JDK_SOURCES        a root with the JDK's sources by module (java.base/, java.logging/, ...), where
#                    the jdk-sources step unpacks the modules that the sources use and lists them
#                    in modules.txt (default $MINECRAFT_CACHE/jdk-src)
# SRC_ZIP            the JDK's src.zip that the jdk-sources step unpacks the modules from (default:
#                    lib/src.zip of the JDK whose java is on PATH)
# JAVA               the java that runs the decompiler (default: java on PATH)
# TRANSLATOR         the java-to-rust binary (default: target/release/java-to-rust, built if missing)
# SLOT               a command that every heavy step runs under, such as a build-slot limiter
#                    (default: none)
# MEMORY_CAP         the most memory the census (the translator with its javac) may take, as
#                    systemd's MemoryMax, such as 6G; it is stopped there rather than the machine
#                    running out (default 6G; empty for none, or where systemd-run is missing)
# PREVIOUS_CENSUS    the census that the summary compares the latest with, a directory with a
#                    census.json (default $MINECRAFT_CACHE/census-previous, where the census step
#                    moves the census before it, if it has one)
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cache=${MINECRAFT_CACHE:-$HOME/.cache/java-to-rust-targets/minecraft-26.3}
jar=${MINECRAFT_JAR:-$cache/server-26.3.jar}
vineflower=${VINEFLOWER:-$cache/vineflower-1.12.0.jar}
vineflower_sha1=85570609a0a5941a7d2918b6260b209de810f66f
cfr=${CFR:-$cache/cfr-0.152.jar}
cfr_sha1=48ef4892cfe8feffddbbd0ff077735140557db74
jdk_sources=${JDK_SOURCES:-$cache/jdk-src}
java=${JAVA:-java}
translator=${TRANSLATOR:-${CARGO_TARGET_DIR:-$root/target}/release/java-to-rust}
read -r -a slot <<< "${SLOT:-}"
memory_cap=${MEMORY_CAP-6G}
previous=${PREVIOUS_CENSUS:-$cache/census-previous}

# Unpacks the bundler jar: the server jar is META-INF/versions/<v>/server-<v>.jar, its libraries
# are under META-INF/libraries, and META-INF/main-class names the class the bundler starts.
extract() {
  [ -f "$jar" ] || { echo "no server jar at $jar" >&2; exit 1; }
  rm -rf "$cache/jar" && mkdir -p "$cache/jar"
  (cd "$cache/jar" && unzip -q "$jar")
  echo "main class: $(cat "$cache/jar/META-INF/main-class")"
  echo "$(find "$cache/jar/META-INF/libraries" -name '*.jar' | wc -l) library jars"
}

# Fetches a jar from Maven Central into the cache, unless it is there, and checks its SHA-1.
fetch_tool() {
  local path=$1 sha1=$2 out=$3
  [ -f "$out" ] || curl -fsS -o "$out" "https://repo1.maven.org/maven2/$path" \
    || { rm -f "$out"; echo "could not fetch $path from Maven Central" >&2; exit 1; }
  echo "$sha1  $out" | sha1sum -c --quiet
}

check_vineflower() {
  fetch_tool org/vineflower/vineflower/1.12.0/vineflower-1.12.0.jar "$vineflower_sha1" "$vineflower"
}

# The library jars, one per line.
library_jars() {
  find "$cache/jar/META-INF/libraries" -name '*.jar' | LC_ALL=C sort
}

# Deletes everything but the Java sources from a decompiler's output (Vineflower copies the jar's
# resources too, CFR writes a summary).
keep_java() {
  find "$1" -type f ! -name '*.java' -delete
  find "$1" -type d -empty -delete
  echo "$(find "$1" -name '*.java' | wc -l) source files from $(basename "$2") ($3)"
}

# Decompiles one jar into a directory with Vineflower, keeping only the Java sources. The flags
# are those haste uses: generic signatures, synthetic members renamed. With a third argument of yes, every other
# library jar is given to Vineflower as a library (-e), so that it knows the generic signatures of
# the classes the code uses. Without them it cannot tell where a generic call needs its type
# arguments written out (`RecordCodecBuilder.<T>mapCodec(...)`, `ImmutableList.<T>builder()`) and
# types variables as Object, and javac then infers other types than the original's. Vineflower
# reads options only before the first -e or --only argument.
decompile_jar() {
  local in=$1 out=$2 with_libs=$3
  shift 3
  local -a libs=()
  local lib
  if [ "$with_libs" = yes ]; then
    while IFS= read -r lib; do
      [ "$lib" = "$in" ] || libs+=("-e=$lib")
    done < <(library_jars)
  fi
  rm -rf "$out" && mkdir -p "$out"
  "${slot[@]}" nice -n 19 "$java" -Xmx3g -jar "$vineflower" -dgs=1 -rsy=1 -log=WARN -thr=4 "$@" "${libs[@]}" \
    "$in" "$out" > "$out.log" 2>&1 || { echo "Vineflower failed on $in (see $out.log)" >&2; return 1; }
  keep_java "$out" "$in" "Vineflower, libraries: $with_libs"
}

# Decompiles the classes of a jar whose names FILTER matches with CFR, the library jars on its
# class path.
decompile_cfr() {
  local in=$1 out=$2 filter=$3
  rm -rf "$out" && mkdir -p "$out"
  "${slot[@]}" nice -n 19 "$java" -Xmx3g -jar "$cfr" "$in" --outputdir "$out" --silent true \
    --jarfilter "$filter" --extraclasspath "$(library_jars | paste -sd:)" \
    > "$out.log" 2>&1 || { echo "CFR failed on $in (see $out.log)" >&2; return 1; }
  keep_java "$out" "$in" CFR
}

# javac on the files listed in LIST (paths relative to DIR, where it runs), every error into LOG.
# --should-stop=ifError=FLOW makes it attribute every file even when one does not parse, so that
# a syntax error in one file does not hide the errors of the others.
javac_errors() {
  local dir=$1 list=$2 classpath=$3 log=$4 classes
  classes=$(mktemp -d)
  (cd "$dir" && "${slot[@]}" javac -J-Xmx3g -proc:none -nowarn -Xmaxerrs 100000 -encoding UTF-8 \
    --should-stop=ifError=FLOW -d "$classes" -cp "$classpath" @"$list") > "$log" 2>&1 || true
  rm -rf "$classes"
}

# The number of javac errors in each file of a javac log, as "count<TAB>file", sorted by file.
error_counts() {
  grep ': error: ' "$1" | cut -d: -f1 | LC_ALL=C sort | uniq -c | awk '{print $1 "\t" $2}'
}

# The server: net.minecraft and com.mojang, decompiled three ways into decompiled/<setup>:
# Vineflower with the library jars, Vineflower without them, and CFR. CFR, which takes 25
# minutes on the whole server, only decompiles the classes of the files that have javac errors
# with the first (compiled all together against the library jars, as the parse step does).
decompile() {
  check_vineflower
  fetch_tool org/benf/cfr/0.152/cfr-0.152.jar "$cfr_sha1" "$cfr"
  local server dir=$cache/decompiled
  server=$(find "$cache/jar/META-INF/versions" -name 'server-*.jar' | head -n 1)
  decompile_jar "$server" "$dir/vineflower" yes --only=net/minecraft --only=com/mojang
  decompile_jar "$server" "$dir/vineflower-nolibs" no --only=net/minecraft --only=com/mojang
  (cd "$dir/vineflower" && find . -name '*.java' | sed 's|^\./||' | LC_ALL=C sort) > "$dir/all-files.txt"
  javac_errors "$dir/vineflower" "$dir/all-files.txt" "$(library_jars | paste -sd:)" "$dir/vineflower-javac.txt"
  error_counts "$dir/vineflower-javac.txt" > "$dir/vineflower-errors.txt"
  # The top-level classes of those files, and their nested classes.
  decompile_cfr "$server" "$dir/cfr" "^($(cut -f2 "$dir/vineflower-errors.txt" | sed -E 's|\.java$||; s|/|\\.|g' \
    | paste -sd'|'))(\\$.*)?\$"
}

# src/server from the three decompiles, file by file: Vineflower with the libraries, except
# where another setup's file gives the whole tree fewer javac errors (decompile-best-of.py), in at
# most 8 compiles of the whole server.
best-of() {
  python3 -I "$root/scripts/decompile-best-of.py" "$cache/decompiled" "$cache/src/server" \
    "$(library_jars | paste -sd:)" 8 vineflower vineflower-nolibs cfr -- "${slot[@]}"
}

# Each library jar into src/libraries/<artifact>, except the native-only ones (a single class
# that loads the native library).
libraries() {
  check_vineflower
  mkdir -p "$cache/src/libraries"
  find "$cache/jar/META-INF/libraries" -name '*.jar' | LC_ALL=C sort | while IFS= read -r lib; do
    local name
    name=$(basename "$lib" .jar)
    case $name in *-linux-* | *-osx-* | *-windows-*) continue ;; esac
    decompile_jar "$lib" "$cache/src/libraries/$name" yes
  done
}

# The published sources of each library that has them on Maven Central, into
# src/library-sources/<artifact> (only the Java files). The bundler keeps its libraries in Maven's
# layout, so each sources jar sits next to where the jar's path says; Mojang's own libraries
# (authlib, brigadier, datafixerupper, jtracy, logging) are not there and stay decompiled.
library-sources() {
  local maven=https://repo1.maven.org/maven2
  mkdir -p "$cache/source-jars" "$cache/src/library-sources"
  (cd "$cache/jar/META-INF/libraries" && find . -name '*.jar' | LC_ALL=C sort) | while IFS= read -r lib; do
    local name path jar out
    name=$(basename "$lib" .jar)
    case $name in *-linux-* | *-osx-* | *-windows-*) continue ;; esac
    path=${lib#./}
    jar=$cache/source-jars/$name-sources.jar
    if [ ! -f "$jar" ]; then
      curl -fsS -o "$jar.sha1" "$maven/${path%.jar}-sources.jar.sha1" 2>/dev/null \
        || { rm -f "$jar.sha1"; echo "no published sources for $name"; continue; }
      curl -fsS -o "$jar" "$maven/${path%.jar}-sources.jar"
    fi
    echo "$(cut -c1-40 "$jar.sha1")  $jar" | sha1sum -c --quiet
    out=$cache/src/library-sources/$name
    rm -rf "$out" && mkdir -p "$out"
    unzip -q "$jar" '*.java' -d "$out"
    echo "$(find "$out" -name '*.java' | wc -l) source files from $(basename "$jar")"
  done
}

# The source roots of the translation's libraries: each library's published sources, or its
# decompiled ones where none are published, and each JDK module.
library_args() {
  local dir name
  for dir in "$cache"/src/libraries/*/; do
    name=$(basename "$dir")
    [ -d "$cache/src/library-sources/$name" ] && dir=$cache/src/library-sources/$name
    printf -- '--library\n%s\n' "${dir%/}"
  done
  local module
  while IFS= read -r module; do
    printf -- '--library\n%s\n' "$jdk_sources/$module"
  done < "$jdk_sources/modules.txt"
}

# The JDK's sources of the modules that the server and its libraries use, from its src.zip:
# java.base, each module with a package that their code names, and the modules that those need
# in turn (scripts/jdk-modules.py), listed in modules.txt.
jdk-sources() {
  local zip=${SRC_ZIP:-$(dirname "$(dirname "$(readlink -f "$(command -v java)")")")/lib/src.zip}
  [ -f "$zip" ] || { echo "no src.zip at $zip (set SRC_ZIP)" >&2; exit 1; }
  mkdir -p "$jdk_sources"
  local -a roots=()
  local dir
  for dir in "$cache/src/server" "$cache/src/libraries" "$cache/src/library-sources"; do
    [ -d "$dir" ] && roots+=("$dir")
  done
  python3 -I "$root/scripts/jdk-modules.py" "$zip" "${roots[@]}" > "$jdk_sources/modules.txt"
  local module
  while IFS= read -r module; do
    rm -rf "${jdk_sources:?}/$module"
    unzip -q "$zip" "$module/*" -d "$jdk_sources"
  done < "$jdk_sources/modules.txt"
  echo "$(wc -l < "$jdk_sources/modules.txt") JDK modules: $(paste -sd' ' "$jdk_sources/modules.txt")"
  echo "$(find "$jdk_sources" -name '*.java' | wc -l) JDK source files"
}

# Compiles every decompiled source with javac, as the translator's front end would, and counts
# the errors by kind (the decompiler's output is not fixed by hand).
parse() {
  local out=$cache/parse
  rm -rf "$out" && mkdir -p "$out/classes"
  find "$cache/src/server" -name '*.java' > "$out/server-files.txt"
  local path
  path=$(find "$cache/src/libraries" -mindepth 1 -maxdepth 1 -type d | paste -sd:)
  # Against the library jars as compiled (their own decompiled sources are checked separately).
  local classpath
  classpath=$(find "$cache/jar/META-INF/libraries" -name '*.jar' | paste -sd:)
  "${slot[@]}" javac -J-Xmx3g -proc:none -nowarn -Xmaxerrs 100000 -encoding UTF-8 --should-stop=ifError=FLOW -d "$out/classes" \
    -cp "$classpath" @"$out/server-files.txt" > "$out/server-javac.txt" 2>&1 || true
  summarize_errors "$out/server-javac.txt" > "$out/server-errors.txt"
  echo "server: $(grep -c ': error: ' "$out/server-javac.txt" || true) errors in" \
    "$(grep ': error: ' "$out/server-javac.txt" | cut -d: -f1 | sort -u | wc -l) of" \
    "$(wc -l < "$out/server-files.txt") files"
  head -n 25 "$out/server-errors.txt"
  : > "$out/libraries-errors.txt"
  for dir in "$cache"/src/libraries/*/; do
    local name
    name=$(basename "$dir")
    find "$dir" -name '*.java' > "$out/$name-files.txt"
    [ -s "$out/$name-files.txt" ] || continue
    "${slot[@]}" javac -J-Xmx3g -proc:none -nowarn -Xmaxerrs 100000 -encoding UTF-8 --should-stop=ifError=FLOW \
      -d "$out/classes-$name" -cp "$classpath" @"$out/$name-files.txt" > "$out/$name-javac.txt" 2>&1 || true
    echo "$name: $(grep -c ': error: ' "$out/$name-javac.txt" || true) errors in" \
      "$(grep ': error: ' "$out/$name-javac.txt" | cut -d: -f1 | sort -u | wc -l) of" \
      "$(wc -l < "$out/$name-files.txt") files" | tee -a "$out/libraries-errors.txt"
    rm -rf "$out/classes-$name"
  done
  rm -rf "$out/classes"
}

# javac's errors by message, with the variable parts (names, types) taken out.
summarize_errors() {
  grep ': error: ' "$1" | sed -E 's/^.*: error: //' \
    | sed -E "s/'[^']*'/'_'/g; s/(symbol|variable|method|class|constructor) [A-Za-z0-9_\$.<>]+/\1 _/g; s/[A-Za-z0-9_\$.]+<[^>]*>/_/g" \
    | sort | uniq -c | sort -rn
}

# The translator's census of the server, past every rejection and every compile error in the
# decompiled sources (--keep-going), with its time and peak memory. Only the main class's file is
# given: the rest of the server is a library root like the others, so javac reads and attributes
# only the classes that the code reachable from main uses. The census before, if it wrote its JSON,
# is moved to census-previous for the summary to compare with; the one before that is deleted.
census() {
  [ -x "$translator" ] || (cd "$root" && "${slot[@]}" cargo build -q --release -p java-to-rust)
  local out=$cache/census
  if [ -f "$out/census.json" ]; then
    rm -rf "$cache/census-previous"
    mv "$out" "$cache/census-previous"
  fi
  mkdir -p "$out"
  [ -f "$jdk_sources/modules.txt" ] || { echo "no $jdk_sources/modules.txt (run the jdk-sources step)" >&2; exit 1; }
  local -a libraries
  mapfile -t libraries < <(library_args)
  local main
  main=$(cat "$cache/jar/META-INF/main-class")
  # --timings prints each phase's time and the peak memory of the translator and of javac.
  local start
  start=$(date +%s)
  local -a cap=()
  if [ -n "$memory_cap" ] && command -v systemd-run > /dev/null; then
    cap=(systemd-run --user --scope --quiet -p "MemoryMax=$memory_cap" -p MemorySwapMax=0)
  fi
  set +e
  "${slot[@]}" "${cap[@]}" "$translator" --timings --main "$main" "$cache/src/server/${main//.//}.java" \
    --library "$cache/src/server" "${libraries[@]}" \
    --census --keep-going --census-json "$out/census.json" --modules-json "$out/modules.json" \
    > "$out/census.txt" 2> "$out/stderr.txt"
  local code=$?
  set -e
  echo "exit $code after $(($(date +%s) - start)) s (with the wait for a slot)" | tee "$out/run.txt"
  grep '^timing: ' "$out/stderr.txt" | tail -n 4
  grep '^javac error: ' "$out/stderr.txt" | sed -E 's/^javac error: [^ ]+ ([^:]+):.*/\1/' | sort | uniq -c \
    | sort -rn > "$out/javac-errors.txt" || true
  echo "$(grep -c '^javac error: ' "$out/stderr.txt" || true) compile errors in the code that javac attributed"
  head -n 40 "$out/census.txt"
}

# The census split by origin: the server, each library, the JDK (census-by-origin.py), and
# compared with the previous census, if one is kept (census-compare.py).
summary() {
  local -a roots=("server=$cache/src/server")
  local dir
  for dir in "$cache"/src/libraries/*/; do
    dir=${dir%/}
    roots+=("$(basename "$dir" | sed -E 's/-[0-9][0-9.]*(-jre|\.Final)?$//')=$dir")
  done
  local module
  while IFS= read -r module; do
    roots+=("JDK=$jdk_sources/$module")
  done < "$jdk_sources/modules.txt"
  python3 -I "$root/scripts/census-by-origin.py" "$cache/census/census.json" "${roots[@]}" \
    | tee "$cache/census/by-origin.txt"
  if [ -f "$previous/census.json" ]; then
    echo
    python3 -I "$root/scripts/census-compare.py" "$previous" "$cache/census" "${roots[@]}" \
      | tee "$cache/census/compare.txt"
  fi
}

steps=("$@")
[ ${#steps[@]} -gt 0 ] || steps=(extract decompile best-of libraries library-sources jdk-sources parse census summary)
for step in "${steps[@]}"; do
  case $step in
    extract | decompile | best-of | libraries | library-sources | jdk-sources | parse | census | summary) "$step" ;;
    *) echo "unknown step $step (extract, decompile, best-of, libraries, library-sources, jdk-sources, parse, census, summary)" >&2
      exit 2 ;;
  esac
done
