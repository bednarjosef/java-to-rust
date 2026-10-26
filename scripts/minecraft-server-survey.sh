#!/usr/bin/env bash
# Prepares the Minecraft: Java Edition dedicated server as a translation target and surveys how
# far the translator gets on it (see docs/minecraft-server.md).
#
#   scripts/minecraft-server-survey.sh [STEP...]      # all steps, or the ones named
#
# Steps: extract, decompile, best-of, libraries, library-sources, library-best-of, repair, jdk-sources,
# parse, census, summary, and on their own (not run by default) trial and java-run.
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
# PROCYON            a directory with the Procyon 0.6.0 decompiler's jars, procyon-compilertools and
#                    procyon-core (default $MINECRAFT_CACHE/decompilers; fetched from Maven Central,
#                    org.bitbucket.mstrobel, if missing; SHA-1s below)
# JDK_SOURCES        a root with the JDK's sources by module (java.base/, java.logging/, ...), where
#                    the jdk-sources step unpacks the modules that the sources use and lists them
#                    in modules.txt (default $MINECRAFT_CACHE/jdk-src)
# SRC_ZIP            the JDK's src.zip that the jdk-sources step unpacks the modules from (default:
#                    lib/src.zip of the JDK whose java is on PATH)
# JAVA               the java that runs the decompiler and the repair (default: java on PATH)
# REPAIR_ROUNDS      how many times the repair step compiles and edits each tree (default 10)
# TRANSLATOR         the java-to-rust binary (default: target/release/java-to-rust, built if missing)
# SLOT               a command that every heavy step runs under, such as a build-slot limiter
#                    (default: none)
# MEMORY_CAP         the most memory the census (the translator with its javac) may take, as
#                    systemd's MemoryMax, such as 6G; it is stopped there rather than the machine
#                    running out (default 6G; empty for none, or where systemd-run is missing)
# TRIAL_CRATES       with a number, the census also writes its trial translation (each rejected
#                    piece ends the program with the rejection) to $MINECRAFT_CACHE/trial, as a
#                    workspace with that many body crates (--crates); the trial step builds and
#                    runs it (default: none)
# TRIAL_MEMORY_CAP   the most memory the trial step's build may take, as MEMORY_CAP (default 16G),
#                    with CARGO_BUILD_JOBS crates built at once (default 10)
# TRIAL_PROFILE      the Cargo profile the trial step builds with (default dev, which takes the least
#                    memory and time to build)
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
procyon=${PROCYON:-$cache/decompilers}
procyon_compilertools_sha1=44c59529227960fec1be237ac97304303caf0ba3
procyon_core_sha1=8f5a33f81d7027d11f1eeb609d9f467f97454c1c
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

check_procyon() {
  mkdir -p "$procyon"
  fetch_tool org/bitbucket/mstrobel/procyon-compilertools/0.6.0/procyon-compilertools-0.6.0.jar \
    "$procyon_compilertools_sha1" "$procyon/procyon-compilertools-0.6.0.jar"
  fetch_tool org/bitbucket/mstrobel/procyon-core/0.6.0/procyon-core-0.6.0.jar \
    "$procyon_core_sha1" "$procyon/procyon-core-0.6.0.jar"
}

# Decompiles the top-level classes of a jar that the files listed in LIST hold (paths such as
# net/minecraft/util/Util.java) with Procyon (scripts/DecompileProcyon.java), the other library
# jars on its class path.
decompile_procyon() {
  local in=$1 out=$2 list=$3
  rm -rf "$out" && mkdir -p "$out"
  sed 's|\.java$||' "$list" | xargs "${slot[@]}" nice -n 19 "$java" -Xmx3g \
    -cp "$procyon/procyon-compilertools-0.6.0.jar:$procyon/procyon-core-0.6.0.jar" \
    "$root/scripts/DecompileProcyon.java" "$in" "$(other_jars "$in")" "$out" > "$out.log" 2>&1 \
    || { echo "Procyon failed on $in (see $out.log)" >&2; return 1; }
  keep_java "$out" "$in" Procyon
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
  { grep ': error: ' "$1" || true; } | cut -d: -f1 | LC_ALL=C sort | uniq -c | awk '{print $1 "\t" $2}'
}

# The library jars that library-sources found no published sources for, except the native-only
# ones (a single class that loads the native library): those stay decompiled. None before the
# library-sources step has run.
unpublished_jars() {
  local lib name
  [ -d "$cache/src/library-sources" ] || { echo "no src/library-sources (run the library-sources step)" >&2; return 0; }
  library_jars | while IFS= read -r lib; do
    name=$(basename "$lib" .jar)
    case $name in *-linux-* | *-osx-* | *-windows-*) continue ;; esac
    [ -d "$cache/src/library-sources/$name" ] || echo "$lib"
  done
}

# The library jars other than JAR, as a class path.
other_jars() {
  library_jars | grep -vxF "$1" | paste -sd:
}

# Decompiles a jar four ways into DIR/<setup>: Vineflower with the library jars, Vineflower
# without them, CFR and Procyon; the rest of the arguments go to Vineflower. CFR, which takes 25
# minutes on the whole server, and Procyon only decompile the classes of the files that have javac
# errors with the first (compiled all together against the other library jars, as the parse step
# does), and are not run where there are none.
decompile_setups() {
  local in=$1 dir=$2
  shift 2
  decompile_jar "$in" "$dir/vineflower" yes "$@"
  decompile_jar "$in" "$dir/vineflower-nolibs" no "$@"
  (cd "$dir/vineflower" && find . -name '*.java' | sed 's|^\./||' | LC_ALL=C sort) > "$dir/all-files.txt"
  javac_errors "$dir/vineflower" "$dir/all-files.txt" "$(other_jars "$in")" "$dir/vineflower-javac.txt"
  error_counts "$dir/vineflower-javac.txt" > "$dir/vineflower-errors.txt"
  rm -rf "$dir/cfr" "$dir/procyon"
  [ -s "$dir/vineflower-errors.txt" ] || return 0
  cut -f2 "$dir/vineflower-errors.txt" > "$dir/error-files.txt"
  decompile_procyon "$in" "$dir/procyon" "$dir/error-files.txt"
  # The top-level classes of those files, and their nested classes.
  decompile_cfr "$in" "$dir/cfr" "^($(cut -f2 "$dir/vineflower-errors.txt" | sed -E 's|\.java$||; s|/|\\.|g' \
    | paste -sd'|'))(\\$.*)?\$"
}

# OUT from the four decompiles of a jar in DIR, file by file: Vineflower with the libraries,
# except where another setup's file gives the whole tree fewer javac errors
# (decompile-best-of.py), in at most 8 compiles of the whole tree.
best_of_setups() {
  local in=$1 dir=$2 out=$3
  python3 -I "$root/scripts/decompile-best-of.py" "$dir" "$out" "$(other_jars "$in")" 8 \
    vineflower vineflower-nolibs cfr procyon -- "${slot[@]}"
}

server_jar() {
  find "$cache/jar/META-INF/versions" -name 'server-*.jar' | head -n 1
}

# The server: net.minecraft and com.mojang, decompiled four ways into decompiled/<setup>.
decompile() {
  check_vineflower
  check_procyon
  fetch_tool org/benf/cfr/0.152/cfr-0.152.jar "$cfr_sha1" "$cfr"
  decompile_setups "$(server_jar)" "$cache/decompiled" --only=net/minecraft --only=com/mojang
}

# src/server from the server's four decompiles.
best-of() {
  best_of_setups "$(server_jar)" "$cache/decompiled" "$cache/src/server"
}

# Edits a decompiled tree where javac rejects it, compiled against CLASSPATH, with edits that keep
# the bytecode's behaviour: casts the decompiler left out (checked ones only where the class file
# in the jar CLASSES has the check), the type arguments of calls javac cannot infer, misnamed
# catch variables (scripts/RepairDecompile.java). Each edit is listed in LOG.
repair_tree() {
  local dir=$1 classpath=$2 log=$3 classes=$4
  local -a exports=()
  local package
  for package in api code model tree util; do
    exports+=(--add-exports "jdk.compiler/com.sun.tools.javac.$package=ALL-UNNAMED")
  done
  "${slot[@]}" nice -n 19 "$java" -Xmx4g "${exports[@]}" "$root/scripts/RepairDecompile.java" "$dir" "$classpath" \
    "${REPAIR_ROUNDS:-10}" "$log" "$classes"
}

# src/server repaired against the library jars, into repair-log.txt, and each library that has
# no published sources (src/libraries/<artifact>) against the other library jars, into
# repair-log-<artifact>.txt. Then the edits made by hand in the cache, if any:
# hand-fixes/server.patch, applied in src/server (it is never part of this repository).
repair() {
  repair_tree "$cache/src/server" "$(library_jars | paste -sd:)" "$cache/repair-log.txt" "$(server_jar)"
  if [ -f "$cache/hand-fixes/server.patch" ]; then
    # A hunk that the repair already made is skipped (and saved as a .rej file beside its file),
    # which patch reports as a failure; applied.txt lists it.
    patch -p1 -N -d "$cache/src/server" < "$cache/hand-fixes/server.patch" > "$cache/hand-fixes/applied.txt" \
      || echo "hand fixes: not all applied, see hand-fixes/applied.txt"
    echo "hand fixes: $(grep -c '^patching' "$cache/hand-fixes/applied.txt") files"
  fi
  unpublished_jars | while IFS= read -r lib; do
    local name
    name=$(basename "$lib" .jar)
    echo "$name:"
    repair_tree "$cache/src/libraries/$name" "$(other_jars "$lib")" "$cache/repair-log-$name.txt" "$lib" < /dev/null
  done
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

# Each library that has no published sources, decompiled four ways into
# decompiled/libraries/<artifact>/<setup> and put together into src/libraries/<artifact> as the
# server is, compiled against the other library jars.
library-best-of() {
  check_vineflower
  check_procyon
  fetch_tool org/benf/cfr/0.152/cfr-0.152.jar "$cfr_sha1" "$cfr"
  unpublished_jars | while IFS= read -r lib; do
    local name
    name=$(basename "$lib" .jar)
    echo "$name:"
    decompile_setups "$lib" "$cache/decompiled/libraries/$name" < /dev/null
    best_of_setups "$lib" "$cache/decompiled/libraries/$name" "$cache/src/libraries/$name" < /dev/null
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
# The server's class files and its libraries' have the tables of their locals (--named-locals),
# which the messages of a NullPointerException show by name.
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
  local -a cap trial=()
  mapfile -t cap < <(memory_cap_command "$memory_cap")
  if [ -n "${TRIAL_CRATES:-}" ]; then
    rm -rf "$cache/trial"
    trial=(--crate "$cache/trial" --crates "$TRIAL_CRATES")
  fi
  set +e
  "${slot[@]}" "${cap[@]}" "$translator" --timings --main "$main" "$cache/src/server/${main//.//}.java" \
    --library "$cache/src/server" "${libraries[@]}" \
    --census --keep-going --named-locals --census-json "$out/census.json" --modules-json "$out/modules.json" \
    "${trial[@]}" > "$out/census.txt" 2> "$out/stderr.txt"
  local code=$?
  set -e
  echo "exit $code after $(($(date +%s) - start)) s (with the wait for a slot)" | tee "$out/run.txt"
  grep '^timing: ' "$out/stderr.txt" | tail -n 4
  grep '^javac error: ' "$out/stderr.txt" | sed -E 's/^javac error: [^ ]+ ([^:]+):.*/\1/' | sort | uniq -c \
    | sort -rn > "$out/javac-errors.txt" || true
  echo "$(grep -c '^javac error: ' "$out/stderr.txt" || true) compile errors in the code that javac attributed"
  head -n 40 "$out/census.txt"
}

# The command that runs another under a memory cap of CAP (one word per line), or none.
memory_cap_command() {
  if [ -n "$1" ] && command -v systemd-run > /dev/null; then
    printf '%s\n' systemd-run --user --scope --quiet -p "MemoryMax=$1" -p MemorySwapMax=0
  fi
}

# Waits up to 600 s for the server in the current directory to log that it is done starting, then
# prints `stop`.
stop_when_started() {
  local waited=0
  until grep -q 'Done (' stdout.txt 2>/dev/null || [ $waited -ge 600 ]; do
    sleep 1
    waited=$((waited + 1))
  done
  echo stop
}

# The server's first start in DIR, a new directory with the EULA accepted (as eula.txt), run as
# COMMAND...: it makes a world and stops at the "stop" that it reads on standard input once
# started, printing the console's log to DIR/stdout.txt. At most 10 minutes.
run_server() {
  local dir=$1
  shift
  rm -rf "$dir" && mkdir -p "$dir"
  echo eula=true > "$dir/eula.txt"
  local start code
  start=$(date +%s)
  set +e
  # `stop` goes in once the server is done starting: the server fails one that comes earlier
  # ("An unexpected error occurred while trying to execute that command") and keeps running.
  (cd "$dir" && stop_when_started | timeout 600 "$@" --nogui > stdout.txt 2> stderr.txt)
  code=$?
  set -e
  echo "exit $code after $(($(date +%s) - start)) s" | tee "$dir/run.txt"
  tail -n 5 "$dir/stdout.txt" "$dir/stderr.txt"
}

# Builds the census's trial translation (TRIAL_CRATES) in trial-target and runs the server with it
# in trial-run.
trial() {
  [ -f "$cache/trial/Cargo.toml" ] || { echo "no trial translation (run the census with TRIAL_CRATES)" >&2; exit 1; }
  local -a cap
  mapfile -t cap < <(memory_cap_command "${TRIAL_MEMORY_CAP:-16G}")
  local start
  start=$(date +%s)
  (cd "$cache/trial" && CARGO_TARGET_DIR=$cache/trial-target CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4} \
    "${slot[@]}" "${cap[@]}" cargo build -q --keep-going --profile "${TRIAL_PROFILE:-dev}" > "$cache/trial-build.txt" 2>&1) \
    || { echo "the trial translation does not build (see trial-build.txt)" >&2; tail -n 20 "$cache/trial-build.txt"; exit 1; }
  echo "built in $(($(date +%s) - start)) s"
  local name profile=${TRIAL_PROFILE:-dev}
  [ "$profile" = dev ] && profile=debug
  name=$(sed -n 's/^members = \[.*, "\([^"]*\)"\]$/\1/p' "$cache/trial/Cargo.toml")
  run_server "$cache/trial-run" "$cache/trial-target/$profile/$name"
}

# The server on Java, from the jars, in java-run: what the trial's run is compared with.
java-run() {
  local classpath
  classpath=$(server_jar):$(library_jars | paste -sd:)
  run_server "$cache/java-run" "$java" -cp "$classpath" "$(cat "$cache/jar/META-INF/main-class")"
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
[ ${#steps[@]} -gt 0 ] || steps=(extract decompile best-of libraries library-sources library-best-of repair jdk-sources
  parse census summary)
for step in "${steps[@]}"; do
  case $step in
    extract | decompile | best-of | libraries | library-sources | library-best-of | repair | jdk-sources | parse \
      | census | summary | trial | java-run) "$step" ;;
    *) echo "unknown step $step (extract, decompile, best-of, libraries, library-sources, library-best-of, repair," \
      "jdk-sources, parse, census, summary, trial, java-run)" >&2
      exit 2 ;;
  esac
done
