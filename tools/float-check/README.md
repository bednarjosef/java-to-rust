# Float and double printing check

A differential check of the runtime's printing against Java. Java's `Float.toString` and
`Double.toString` (the JDK's own algorithm) and `float_to_string` and `double_to_string` in
`crates/runtime` print the same bit patterns; the script compares the two line by line and
reports every value that prints differently.

The runtime formats `float` and `double` the same way `Double.toString` does, so
`String.valueOf`, string concatenation and `println` of those types all go through these two
functions. The check therefore covers them too.

## Running it

```
tools/float-check/run.sh              # edge values, both widths (default)
tools/float-check/run.sh random       # 5,000,000 random bit patterns per width
tools/float-check/run.sh random 1000000 7   # COUNT and SEED
tools/float-check/run.sh sweep        # every float bit pattern, all 2^32
tools/float-check/run.sh all          # edge, random, then sweep
```

It needs `java` (the JDK; the check was run with OpenJDK 26, which runs `FloatCheck.java`
directly) and `cargo`. The script builds the two release binaries in `rust/` first. `rust/` is a
standalone crate with its own `[workspace]`, so it stays out of the root workspace and uses
`crates/runtime` by path.

Exit status is 0 when every value matches, 1 when any value prints differently (the first 20
mismatches of each run are printed, as `Java prints X, runtime prints Y`), and 2 on a usage or
build error.

## How it works

- `gen` (in `rust/src/bin/gen.rs`) writes the bit patterns, one per line in hex (8 digits for
  `float`, 16 for `double`). It is the only source of inputs, so Java and the runtime always see
  the same values.
- `FloatCheck.java` reads the lines and prints `hex TAB Float.toString(...)`.
- `check` (in `rust/src/bin/check.rs`) reads the same lines and prints `hex TAB float_to_string(...)`
  or `double_to_string(...)`.
- `run.sh` pastes the two outputs side by side and an `awk` script counts the lines that differ.

Values stream through pipes, so the full float sweep never holds more than one chunk's
output in memory.

## What is checked

`edge` (both widths):

- Specials and extremes: `±0`, the smallest and largest normal and subnormal values, `±Infinity`,
  `NaN` and the bit patterns right next to each of them (±2 ulps).
- Decimals with one or two significant digits (`1`–`99`), each with ±2 ulps, at every decimal
  exponent of the width (float `1e-45` to `9e38`, double `5e-324` to `9e308`). Those are the
  cases where Java's "closest of the shortest" rule matters most.
- Decimals with three significant digits (`100` to `999`) at every exponent in range, exact.
- The plain/E notation boundaries: `0.001`, `1e-3`, `1e7`, `9999999`, `9.9999999e6`, `1e-4`
  and their neighbours (±50 ulps).
- Subnormals: the first and the last 2^17 subnormal values, and the values around the smallest
  normal one (±50 ulps).
- Integers from 0 up to 2^24 (float) and 2^20 (double), both signs, and the integers from 1 to
  4096 with the sign bit set.
- Powers of two from 2^0 to the top exponent (±2 ulps), and the integers `2^k - 3` to `2^k + 3`
  for every `k` the width holds exactly (up to 2^31 for float, 2^63 for double).
- 200,000 pseudo-random integers below 2^24 (float) or 2^53 (double).

`random`: uniformly random bit patterns of each width (SplitMix64, seeded), so the values cover
every exponent, including NaN payloads (which both sides print as `NaN`).

`sweep`: every one of the 2^32 float bit patterns, in 16 chunks of 2^28, run in parallel.

## Results and timings

Run on a 16-core machine with OpenJDK 26.0.2 (JDK run time dominates; Java starts once per
chunk, so timings include JVM start-up):

| Run | Values | Mismatches | Time |
|---|---|---|---|
| `edge` (float and double) | 34,720,358 float + 4,804,182 double | 0 | about 54 s |
| `random 5000000 1` (float and double) | 5,000,000 + 5,000,000 | 0 | about 11 s |
| `sweep` (all float bit patterns) | 4,294,967,296 | 0 | about 25 min |

Before the fix in `crates/runtime` (shared shortest-digit selection), `edge` found 72 float
mismatches, all exact ties such as `1100000.25f`, which Java prints as `1100000.2`; the runtime
printed `1100000.3`. The unit tests `ties_between_shortest_decimals_take_the_even_digit` in
`lib.rs` and `float.rs` pin those values.

## Limits

- Double precision is not swept: 2^64 patterns cannot be run. Doubles are covered by the edge
  values, the decimals and boundaries, and 5,000,000 random patterns. The float sweep is the
  exhaustive part.
- The check compares printing only. Arithmetic is not compared, and neither is parsing of
  literals.
- The check needs a JDK on `PATH`. The Rust side is built in release mode on each run, so the
  timings above include a build that is quick once the crate is compiled.
