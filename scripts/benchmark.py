#!/usr/bin/env python3
"""Times the programs in benchmarks/ as Java and as translated Rust.

Each program is compiled with javac, translated by java-to-rust into a Cargo project under
target/benchmarks/, built in release mode and run on both sides. The Rust run must print the
same standard output and exit with the same code as Java's; a program that differs, or that
does not translate or compile, is reported and not timed. Times are the median of the runs,
wall clock: Java with its default flags, JVM start-up included. A program that warms up prints
a line `kernel-ms N` on standard error after its warm-up runs: N is the time of the timed kernel
measured inside the program, and it is reported as well. So is each side's peak memory, the most
resident memory the process held (the median over the runs of its `ru_maxrss`), measured by
`scripts/peak-rss.c`, which runs the program as its own child so that the peak is the program's
alone and not the forked Python's (about 20 MB). Java runs with `-XX:MaxRAM` set to the machine's
memory, so that its heap is sized as at its defaults even under a cap of the memory of a cgroup.

A program that does not translate on its own, as one that uses the JDK's collections, is
translated as a whole program with the JDK's sources when `JAVA_BASE_SOURCES` names the source
root of `java.base`; its name is marked `*` in the table.

    python3 scripts/benchmark.py [--runs N] [Program ...]
"""

import argparse
import os
import re
import shutil
import statistics
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROGRAMS = ROOT / "benchmarks"
OUT = ROOT / "target" / "benchmarks"
KERNEL_MS = re.compile(r"^kernel-ms (\d+)$", re.MULTILINE)
PEAK_RSS = re.compile(r"^peak-rss-kb (\d+)\n", re.MULTILINE)
RUN_TIMEOUT = 600  # seconds; a run that takes longer fails the program
# The machine's memory in bytes, for Java's `-XX:MaxRAM`: inside a cgroup with a memory cap, the
# JVM sizes its heap from the cap (768 MB under a cap of 3 GB, against 7.8 GB on this machine).
MACHINE_MEMORY = next(
    int(l.split()[1]) * 1024 for l in Path("/proc/meminfo").read_text().splitlines() if l.startswith("MemTotal:")
)


@dataclass
class Run:
    stdout: str
    code: int
    stderr: str
    ms: float
    peak_mb: float


@dataclass
class Row:
    name: str
    status: str = "ok"
    java_ms: float = 0.0
    rust_ms: float = 0.0
    java_kernel: float | None = None
    rust_kernel: float | None = None
    java_mb: float = 0.0
    rust_mb: float = 0.0
    whole: bool = False


def crate_name(stem):
    # The translator names the Cargo package, and so the binary, after the file (--crate).
    return re.sub(r"[^a-z0-9]", "-", stem.lower())


def first_error(stderr):
    for line in stderr.splitlines():
        if line.startswith("error") and "could not compile" not in line:
            return line
    return stderr.strip().splitlines()[-1] if stderr.strip() else "failed"


def peak_rss_helper():
    """Builds `scripts/peak-rss.c` (with `cc`, which Rust needs as its linker anyway)."""
    helper = OUT / "peak-rss"
    source = ROOT / "scripts" / "peak-rss.c"
    if not helper.exists() or helper.stat().st_mtime < source.stat().st_mtime:
        OUT.mkdir(parents=True, exist_ok=True)
        subprocess.run(["cc", "-O2", "-o", str(helper), str(source)], check=True)
    return helper


def measured_run(cmd):
    """Runs `cmd` to its end through `peak-rss`: its output, exit code, wall time and peak
    resident memory, which `peak-rss` writes as the last line of standard error."""
    start = time.perf_counter()
    proc = subprocess.Popen([str(HELPER)] + cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    output = {}

    def read(name, stream):
        output[name] = stream.read()

    readers = [threading.Thread(target=read, args=(n, s)) for n, s in (("out", proc.stdout), ("err", proc.stderr))]
    for r in readers:
        r.start()
    timer = threading.Timer(RUN_TIMEOUT, proc.kill)
    timer.start()
    _, status, _ = os.wait4(proc.pid, 0)
    timer.cancel()
    ms = (time.perf_counter() - start) * 1000
    proc.returncode = os.waitstatus_to_exitcode(status)
    for r in readers:
        r.join()
    peaks = PEAK_RSS.findall(output["err"])
    stderr = PEAK_RSS.sub("", output["err"])
    return Run(output["out"], proc.returncode, stderr, ms, int(peaks[-1]) / 1024 if peaks else 0.0)


def timed_runs(cmd, runs):
    return [measured_run(cmd) for _ in range(runs)]


def kernel_median(runs):
    values = [int(m.group(1)) for r in runs if (m := KERNEL_MS.search(r.stderr))]
    return statistics.median(values) if len(values) == len(runs) else None


def benchmark(java_file, translator, runs):
    stem = java_file.stem
    row = Row(stem)
    classes = OUT / "classes" / stem
    crate = OUT / stem / "crate"
    classes.mkdir(parents=True, exist_ok=True)

    javac = subprocess.run(["javac", "-d", str(classes), str(java_file)], capture_output=True, text=True)
    if javac.returncode:
        row.status = f"javac: {first_error(javac.stderr)}"
        return row
    translation = subprocess.run(
        [str(translator), str(java_file), "--crate", str(crate)], capture_output=True, text=True
    )
    library = os.environ.get("JAVA_BASE_SOURCES")
    if translation.returncode and library:
        # Translated whole, with the JDK's sources, from a source root of its own.
        sources = OUT / "sources" / stem
        sources.mkdir(parents=True, exist_ok=True)
        shutil.copy(java_file, sources / java_file.name)
        whole = subprocess.run(
            [str(translator), "--main", stem, "--source", str(sources), "--library", library, "--crate", str(crate)],
            capture_output=True,
            text=True,
        )
        if not whole.returncode:
            translation, row.whole = whole, True
    if translation.returncode:
        row.status = f"does not translate: {translation.stderr.strip()[-400:]}"
        return row
    build = subprocess.run(
        ["cargo", "build", "--release", "-q", "--target-dir", str(OUT / "target")],
        cwd=crate,
        capture_output=True,
        text=True,
    )
    if build.returncode:
        row.status = f"does not compile: {first_error(build.stderr)}"
        return row

    java = timed_runs(["java", f"-XX:MaxRAM={MACHINE_MEMORY}", "-cp", str(classes), stem], runs)
    rust = timed_runs([str(OUT / "target" / "release" / crate_name(stem))], runs)
    expected = (java[0].stdout, java[0].code)
    for run in java + rust:
        if (run.stdout, run.code) != expected:
            row.status = f"output differs from Java (exit {run.code}, Java exit {expected[1]})"
            return row
    row.java_ms = statistics.median(r.ms for r in java)
    row.rust_ms = statistics.median(r.ms for r in rust)
    row.java_kernel = kernel_median(java)
    row.rust_kernel = kernel_median(rust)
    row.java_mb = statistics.median(r.peak_mb for r in java)
    row.rust_mb = statistics.median(r.peak_mb for r in rust)
    return row


def ratio(rust, java):
    return f"{rust / java:.2f}" if rust is not None and java else "-"


def print_table(rows):
    print(
        f"{'program':<16}{'Java ms':>10}{'Rust ms':>10}{'Rust/Java':>11}"
        f"{'Java kernel':>13}{'Rust kernel':>13}{'kernel ratio':>14}{'Java MB':>9}{'Rust MB':>9}"
    )
    for r in rows:
        name = r.name + ("*" if r.whole else "")
        if r.status != "ok":
            print(f"{name:<16}  {r.status}")
            continue
        kernel = (
            f"{r.java_kernel:>13.0f}{r.rust_kernel:>13.0f}{ratio(r.rust_kernel, r.java_kernel):>14}"
            if r.java_kernel is not None and r.rust_kernel is not None
            else f"{'-':>13}{'-':>13}{'-':>14}"
        )
        print(
            f"{name:<16}{r.java_ms:>10.0f}{r.rust_ms:>10.0f}{ratio(r.rust_ms, r.java_ms):>11}{kernel}"
            f"{r.java_mb:>9.1f}{r.rust_mb:>9.1f}"
        )


def environment():
    java = subprocess.run(["java", "-version"], capture_output=True, text=True).stderr.splitlines()[0]
    rustc = subprocess.run(["rustc", "-V"], capture_output=True, text=True).stdout.strip()
    cpu = next(
        (l.split(":", 1)[1].strip() for l in Path("/proc/cpuinfo").read_text().splitlines()
         if l.startswith("model name")),
        "unknown CPU",
    )
    load = os.getloadavg()[0]
    return f"{cpu} ({os.cpu_count()} cores); {java}; {rustc}; load average {load:.1f} at start"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--runs", type=int, default=5, help="runs per program and side (default 5)")
    parser.add_argument("programs", nargs="*", help="program names (default: all of benchmarks/)")
    args = parser.parse_args()

    files = sorted(PROGRAMS.glob("*.java"))
    if args.programs:
        wanted = set(args.programs)
        files = [f for f in files if f.stem in wanted]
        missing = wanted - {f.stem for f in files}
        if missing:
            parser.error(f"no such program: {', '.join(sorted(missing))}")

    print("building java-to-rust", file=sys.stderr)
    subprocess.run(["cargo", "build", "--release", "-q", "-p", "java-to-rust"], cwd=ROOT, check=True)
    # Cargo builds where CARGO_TARGET_DIR says, when it is set.
    translator = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "release" / "java-to-rust"

    global HELPER
    HELPER = peak_rss_helper()
    print(environment())
    print(f"median of {args.runs} runs, wall clock, milliseconds; peak resident memory, MB\n")
    rows = []
    for f in files:
        print(f"running {f.stem}", file=sys.stderr)
        rows.append(benchmark(f, translator, args.runs))
    print_table(rows)
    return 0 if all(r.status == "ok" for r in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
