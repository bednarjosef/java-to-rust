#!/usr/bin/env bash
# Differential check of float and double printing: the runtime (crates/runtime) against
# Java's Float.toString and Double.toString. See README.md.
#
# usage: tools/float-check/run.sh [edge | random [COUNT [SEED]] | sweep | all]
#   edge    the edge values for float and double (default)
#   random  COUNT (default 5000000) pseudo-random bit patterns for each of float and double
#   sweep   all 2^32 float bit patterns, in chunks run in parallel (JOBS, default nproc)
#   all     edge, then random, then sweep
# Exits 1 when any value prints differently. Prints the summaries and up to 20 mismatches each.
set -uo pipefail

here="$(dirname "$0")"
java_src="$here/java/FloatCheck.java"
cargo build --release --quiet --manifest-path "$here/rust/Cargo.toml" --bins || exit 2
gen="$here/rust/target/release/gen"
check="$here/rust/target/release/check"

# compare NAME KIND GEN-ARGS...: generates the bit patterns with gen, prints them through Java
# and through the runtime, and compares the two lines for each value. The last line printed is
# `NAME: N values, M mismatches`.
compare() {
    local name=$1 kind=$2
    shift 2
    paste -d '\t' \
        <("$gen" "$kind" "$@" | java "$java_src" "$kind") \
        <("$gen" "$kind" "$@" | "$check" "$kind") |
        awk -F'\t' -v name="$name" '
            {
                n++
                if ($1 != $3 || $2 != $4) {
                    bad++
                    if (bad <= 20) printf "  MISMATCH %s: Java prints %s, runtime prints %s\n", $1, $2, $4
                }
            }
            END {
                printf "%s: %d values, %d mismatches\n", name, n, bad + 0
                exit (bad > 0)
            }'
}

edge() {
    compare "float edge" float edge
    local f=$?
    compare "double edge" double edge
    return $(( f | $? ))
}

random() {
    local count=${1:-5000000} seed=${2:-1}
    compare "float random $count (seed $seed)" float random "$count" "$seed"
    local f=$?
    compare "double random $count (seed $seed)" double random "$count" "$seed"
    return $(( f | $? ))
}

# Every float bit pattern, 2^28 at a time, each chunk compared in its own pipeline.
sweep() {
    local chunk=$((1 << 28)) jobs=${JOBS:-$(nproc)} dir start running=0 n b values=0 bad=0
    dir="$(mktemp -d)"
    for ((start = 0; start < 1 << 32; start += chunk)); do
        if (( running >= jobs )); then wait -n; running=$((running - 1)); fi
        compare "float sweep $(printf '%08x..%08x' "$start" $((start + chunk - 1)))" float \
            range "$start" "$chunk" >"$dir/$start" 2>&1 &
        running=$((running + 1))
    done
    wait
    for ((start = 0; start < 1 << 32; start += chunk)); do
        cat "$dir/$start"
        read -r n b < <(sed -En 's/.*: ([0-9]+) values, ([0-9]+) mismatches$/\1 \2/p' "$dir/$start")
        values=$((values + n))
        bad=$((bad + b))
    done
    rm -rf "$dir"
    echo "float sweep: $values values (all 2^32 bit patterns), $bad mismatches"
    return $(( bad > 0 ))
}

status=0
case "${1:-edge}" in
    edge) edge; status=$? ;;
    random) random "${2:-}" "${3:-}"; status=$? ;;
    sweep) sweep; status=$? ;;
    all) edge; status=$?; random "" ""; status=$(( status | $? )); sweep; status=$(( status | $? )) ;;
    *) echo "usage: $0 [edge | random [COUNT [SEED]] | sweep | all]" >&2; exit 2 ;;
esac
exit "$status"
