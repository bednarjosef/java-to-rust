#!/usr/bin/env python3
"""Writes a symbol ordering file for the linker from a perf profile of a translated program.

    scripts/hot-text-order.py PERF_DATA > order.txt

PERF_DATA is a `perf record` of the program running its usual work (for the Minecraft server, the
bench step's sprint with BENCH_PROFILE; see docs/minecraft-server.md). Each function that a sample
landed in is hot; the file names them by their symbols, the most sampled first, between the
runtime's markers `j2r_hot_text_start` and `j2r_hot_text_end`. Linked with it
(`--symbol-ordering-file`), the hot functions come first in the program's code, in that order and
next to each other, and the runtime puts the range between the markers on huge pages when the
program starts (crates/runtime/src/hot_text.rs). Symbols of other files (the C library, the
kernel) and samples outside any symbol are left out; the linker ignores names it does not find.
"""

import collections
import subprocess
import sys


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    script = subprocess.run(
        ["perf", "script", "-i", sys.argv[1], "-F", "ip,sym,dso", "--no-demangle"],
        capture_output=True, text=True, check=True,
    ).stdout
    samples = collections.Counter()
    for line in script.splitlines():
        fields = line.split()
        # "ip symbol (dso)"; a symbol has no spaces in its mangled form
        if len(fields) != 3 or not fields[2].startswith("("):
            continue
        dso = fields[2].strip("()")
        if dso.startswith(("/usr/lib", "/lib", "[")):
            continue
        samples[fields[1]] += 1
    print("j2r_hot_text_start")
    for symbol, _ in samples.most_common():
        if symbol not in ("j2r_hot_text_start", "j2r_hot_text_end", "[unknown]"):
            print(symbol)
    print("j2r_hot_text_end")


if __name__ == "__main__":
    main()
