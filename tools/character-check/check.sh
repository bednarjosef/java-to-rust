#!/usr/bin/env bash
# Runs every supported Character method on every char 0..0xFFFF, on the JDK and through the
# runtime, and summarises where they disagree. Output goes to target/character-check/; the full
# list of disagreements is in mismatches.txt there. See README.md.
set -uo pipefail
cd "$(dirname "$0")/../.."

dir=tools/character-check
out=target/character-check
mkdir -p "$out"
rm -f "$out/mismatches.txt"

java "$dir/CharacterSweep.java" chars >"$out/java-chars.txt" || { echo "the JDK sweep failed"; exit 1; }
java "$dir/CharacterSweep.java" for-digit >"$out/java-for-digit.txt" || { echo "the JDK sweep failed"; exit 1; }

cargo build -q --release --manifest-path "$dir/rust/Cargo.toml" --target-dir "$out/cargo" \
    || { echo "the runtime sweep does not build"; exit 1; }
bin="$out/cargo/release/character-check"
"$bin" chars >"$out/rust-chars.txt" || { echo "the runtime sweep failed"; exit 1; }
"$bin" for-digit >"$out/rust-for-digit.txt" || { echo "the runtime sweep failed"; exit 1; }

echo "JDK: $(java -version 2>&1 | head -1)"
echo "Rust: $(rustc --version)"
echo

if ! diff -q <(head -1 "$out/java-chars.txt") <(head -1 "$out/rust-chars.txt") >/dev/null; then
    echo "the headers of the two sweeps differ: fix CharacterSweep.java or rust/src/main.rs"
    exit 1
fi

paste -d ' ' "$out/java-chars.txt" "$out/rust-chars.txt" \
    | awk -v detail="$out/mismatches.txt" -f "$dir/summarise.awk"

forDigit=$(diff "$out/java-for-digit.txt" "$out/rust-for-digit.txt" | grep -c '^<' || true)
echo "forDigit: $forDigit of $(($(wc -l <"$out/java-for-digit.txt") - 1)) (digit, radix) pairs disagree"
diff "$out/java-for-digit.txt" "$out/rust-for-digit.txt" | grep '^[<>]' | head -10 || true
