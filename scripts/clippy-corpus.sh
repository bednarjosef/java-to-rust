#!/usr/bin/env bash
# Counts the clippy warnings in the Rust that the translator generates for every program in
# corpus/. Each program is translated as corpus/tests does, the generated `#![allow]` lines are
# removed so that every lint counts, and `cargo clippy` runs over one Cargo project with a binary
# per program. Prints the warnings per lint, per program and in total, and the programs that do
# not translate or do not compile. The generated code lives in target/clippy-corpus.
#
#     scripts/clippy-corpus.sh
set -uo pipefail
cd "$(dirname "$0")/.."

work=target/clippy-corpus
rm -rf "$work/src" "$work/sources" "$work/Cargo.toml"
mkdir -p "$work/src/bin" "$work/sources"
cargo build -q -p java-to-rust || exit 1
translator=target/debug/java-to-rust
runtime=$(cd crates/runtime && pwd)

cat > "$work/Cargo.toml" <<EOF
[package]
name = "corpus"
version = "0.0.0"
edition = "2024"

[dependencies]
java-runtime = { path = "$runtime" }

[workspace]
EOF

untranslated=()
for java in corpus/*/*.java; do
    group=$(basename "$(dirname "$java")")
    stem=$(basename "$java" .java | tr '[:upper:]' '[:lower:]')
    name="$group-$stem"
    if ! "$translator" "$java" > "$work/src/bin/$name.rs" 2> "$work/sources/$name.err"; then
        untranslated+=("$name")
        rm -f "$work/src/bin/$name.rs"
    fi
done
for dir in corpus/projects/*/; do
    name="projects-$(basename "$dir")"
    main=$(tr -d '[:space:]' < "$dir/main")
    # A library root, and optional classes the translator does not see (as corpus/tests does).
    extra=()
    [ ! -d "$dir/lib" ] || extra+=(--library "$dir/lib")
    [ ! -d "$dir/optional" ] || extra+=(--keep-going)
    if ! "$translator" --main "$main" --source "$dir/src" "${extra[@]}" > "$work/src/bin/$name.rs" 2> "$work/sources/$name.err"; then
        untranslated+=("$name")
        rm -f "$work/src/bin/$name.rs"
    fi
done
translated=$(ls "$work"/src/bin/*.rs | wc -l)

sed -i -E '/^[[:space:]]*#!?\[allow\(/d' "$work"/src/bin/*.rs

(cd "$work" && cargo clippy --bins --keep-going --message-format=json -q -- > clippy.json 2> clippy.err)

# One line per diagnostic of a program: the program, its level, its lint or error code. A rustc
# error (no code, or `E....`) means the program does not compile; a clippy lint set to deny is
# counted as a finding like a warning.
jq -r 'select(.reason == "compiler-message")
    | .message as $m
    | ([$m.spans[] | select(.is_primary) | .file_name][0] // "") as $file
    | select($file | startswith("src/bin/"))
    | [($file | sub("^src/bin/"; "") | sub("\\.rs$"; "")), $m.level, ($m.code.code // "(none)")]
    | @tsv' "$work/clippy.json" > "$work/diagnostics.tsv"

awk -F'\t' '
    $2 == "error" && ($3 == "(none)" || $3 ~ /^E[0-9]+$/) { broken[$1] = 1; next }
    $2 == "warning" || $2 == "error" { per_lint[$3]++; per_program[$1]++; total++ }
    END {
        printf "warnings per lint\n"
        for (l in per_lint) printf "%6d  %s\n", per_lint[l], l | "sort -k1,1nr -k2,2"
        close("sort -k1,1nr -k2,2")
        printf "\nwarnings per program\n"
        for (p in per_program) printf "%6d  %s\n", per_program[p], p | "sort -k1,1nr -k2,2"
        close("sort -k1,1nr -k2,2")
        printf "\n"
        for (p in broken) printf "does not compile: %s\n", p | "sort"
        close("sort")
        printf "total warnings: %d\n", total
    }
' "$work/diagnostics.tsv"

for name in "${untranslated[@]}"; do
    echo "does not translate: $name ($(head -1 "$work/sources/$name.err"))"
done
echo "translated: $translated of $((translated + ${#untranslated[@]})) programs"
