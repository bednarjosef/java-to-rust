#!/usr/bin/env python3
"""Generates programs from seeds, runs each on Java and as translated Rust, and reports.

    python3 tools/fuzz/run.py [--count 200] [--start 1] [--size 30] [--jobs 4] [--strings]
                                [--arrays] [--objects]

Text is read and written as UTF-8 whatever the locale, because the string features print
non-ASCII characters. The one exception is String.format, which uses the default locale.

Each program gets one status:
  match             the translation prints the same stdout and exits with the same code as Java
  mismatch          they differ in stdout, exit code or the uncaught exception's class and
                    message (the detail shows the first difference)
  rustc             the translation does not compile (the detail shows the first error)
  translator-crash  the translator panics or exits without a rejection message
  rust-timeout      the translation runs past the time limit
  rejected          the translator stops with a message such as "Object not supported yet"
  invalid           javac rejects the generated program: a bug in fuzz.py, not the translator
  java-timeout      Java runs past the time limit: also a bug in fuzz.py
  harness-error     something in this script failed; the detail says what

Artifacts go to target/fuzz: runs/<name>/ for programs that did not match, results.jsonl for
every result, crates/<slot>/ for the Cargo projects (one per parallel slot, reused).
"""

import argparse
import json
import os
import queue
import re
import shutil
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fuzz  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
# Two runs at once (say, of two builds of the translator) need their own work directories:
# FUZZ_WORK and FUZZ_TRANSLATOR override these, and --no-build keeps the given translator.
WORK = Path(os.environ.get('FUZZ_WORK', ROOT / 'target' / 'fuzz')).resolve()
TRANSLATOR = Path(os.environ.get('FUZZ_TRANSLATOR', ROOT / 'target' / 'release' / 'java-to-rust')).resolve()
JAVA_TIMEOUT = 30
TRANSLATE_TIMEOUT = 120
BUILD_TIMEOUT = 900
RUST_TIMEOUT = 20
STATUSES = ['match', 'mismatch', 'rustc', 'translator-crash', 'rust-timeout', 'rejected',
            'invalid', 'java-timeout', 'harness-error']
FAILURES = ('mismatch', 'rustc', 'translator-crash', 'rust-timeout')
REJECTION = re.compile(r'^Prog\.java:\d+:\d+: (.*)$', re.M)
UNSUPPORTED = 'not supported yet'  # the translator's own rejections, after javac accepted
RUSTC_ERROR = re.compile(r'^error(?:\[(E\d+)\])?: (.*)$', re.M)
SETUP = 'public class Prog {\n    public static void main(String[] args) {\n    }\n}\n'
UTF8 = {'encoding': 'utf-8', 'errors': 'replace'}  # for subprocess output, as text
JAVA = ['java', '-XX:-UsePerfData', '-Dstdout.encoding=UTF-8', '-Dfile.encoding=UTF-8']


def build_translator():
    """Builds the translator in release mode, as the translation runs need it."""
    subprocess.run(['cargo', 'build', '--release', '-p', 'java-to-rust'], cwd=ROOT, check=True)


def compile_java(run):
    """javac's first error for Prog.java in `run`, or '' when javac accepts the program."""
    (run / 'classes').mkdir(exist_ok=True)
    done = subprocess.run(['javac', '-encoding', 'UTF-8', '-d', run / 'classes', 'Prog.java'],
                          cwd=run, capture_output=True, **UTF8, stdin=subprocess.DEVNULL,
                          timeout=JAVA_TIMEOUT)
    if done.returncode == 0:
        return ''
    return next((ln for ln in done.stderr.splitlines() if ': error: ' in ln), 'javac failed')


EXCEPTION = re.compile(r'^Exception in thread "main" ([\w.$]+)(?:: ?(.*))?$', re.M)


def exception(stderr):
    """The class and message of the uncaught exception in `stderr`, or None. Java's messages
    for NullPointerException describe the expression that was null, which the translation does
    not repeat, so only that class is compared."""
    found = EXCEPTION.search(stderr)
    if found is None:
        return None
    name, message = found.group(1), (found.group(2) or '').strip()
    return (name, '') if name == 'java.lang.NullPointerException' else (name, message)


def describe(java, rust):
    """The first line where the two stdouts differ, and the two exit codes."""
    jl, rl = java.stdout.splitlines(), rust.stdout.splitlines()
    for i in range(max(len(jl), len(rl))):
        a = jl[i] if i < len(jl) else '<end>'
        b = rl[i] if i < len(rl) else '<end>'
        if a != b:
            return f'line {i + 1}: java {a!r}, rust {b!r}; exit {java.returncode} vs {rust.returncode}'
    if java.returncode != rust.returncode:
        return f'exit {java.returncode} vs {rust.returncode}'
    return f'exception: java {exception(java.stderr)}, rust {exception(rust.stderr)}'


class Runner:
    """Classifies programs. Each of `jobs` slots owns one Cargo crate, so builds never collide.

    With `keep` set, the run directory of each failure stays for inspection; the directories of
    other statuses are always removed.
    """

    def __init__(self, jobs, keep=True):
        self.keep = keep
        self.slots = queue.Queue()
        for k in range(jobs):
            self.slots.put(k)
        self.pool = ThreadPoolExecutor(max_workers=jobs)
        self.crates = WORK / 'crates'
        setup = self.crates / 'setup'
        setup.mkdir(parents=True, exist_ok=True)
        (setup / 'Prog.java').write_text(SETUP)
        for k in range(jobs):
            crate = self.crates / str(k)
            if not (crate / 'Cargo.toml').exists():
                crate.mkdir(parents=True, exist_ok=True)
                subprocess.run([TRANSLATOR, 'Prog.java', '--crate', crate], cwd=setup, check=True,
                               stdin=subprocess.DEVNULL, capture_output=True)

    def classify_many(self, items):
        """`items` are (name, source) pairs. Yields the results in the same order."""
        return self.pool.map(lambda item: self.classify(*item), items)

    def classify(self, name, source):
        slot = self.slots.get()
        try:
            return self.run(slot, name, source)
        except Exception as e:  # keep a long run going; the detail says what broke
            return self.finish(WORK / 'runs' / name, name, 'harness-error', repr(e))
        finally:
            self.slots.put(slot)

    def finish(self, run, name, status, detail, key=''):
        if not (self.keep and status in FAILURES):
            shutil.rmtree(run, ignore_errors=True)
        return {'name': name, 'status': status, 'key': key, 'detail': detail}

    def run(self, slot, name, source):
        run = WORK / 'runs' / name
        run.mkdir(parents=True, exist_ok=True)
        (run / 'Prog.java').write_text(source, encoding='utf-8')
        crate = self.crates / str(slot)
        finish = lambda *a, **k: self.finish(run, name, *a, **k)  # noqa: E731

        # Translation: the translator runs javac first, so a program that compiles reaches the
        # translation. A rejection that names an unsupported feature can only come after javac
        # passed; any other failure may be a compile error, which is a generator bug (`invalid`).
        main_rs = crate / 'src' / 'main.rs'
        try:
            tr = subprocess.run([TRANSLATOR, 'Prog.java', '-o', main_rs], cwd=run,
                                capture_output=True, **UTF8, stdin=subprocess.DEVNULL,
                                timeout=TRANSLATE_TIMEOUT)
        except subprocess.TimeoutExpired:
            return finish('translator-crash', 'the translator ran past the time limit')
        if tr.returncode != 0:
            rejected = REJECTION.search(tr.stderr)
            if not (rejected and UNSUPPORTED in rejected.group(1)):
                error = compile_java(run)
                if error:
                    return finish('invalid', error)
            if rejected:
                return finish('rejected', rejected.group(1), key=rejected.group(1))
            first = (tr.stderr.strip().splitlines() or ['no output'])[0]
            return finish('translator-crash', first, key=first)

        # Build: the crate is reused, so only main.rs changed since the last build.
        build = subprocess.run(['cargo', 'build', '--release', '-q'], cwd=crate,
                               capture_output=True, **UTF8, stdin=subprocess.DEVNULL,
                               timeout=BUILD_TIMEOUT)
        if build.returncode != 0:
            error = next((m for m in RUSTC_ERROR.finditer(build.stderr)
                          if 'could not compile' not in m.group(2)), None)
            if error is None:
                return finish('rustc', 'build failed', key='build failed')
            code = error.group(1) or error.group(2)[:60]
            return finish('rustc', f'{error.group(1) or ""} {error.group(2)}'.strip(), key=code)

        # Run the translation first: Java only runs when its output is needed for a verdict.
        package = re.search(r'^name = "(.*)"', (crate / 'Cargo.toml').read_text(), re.M).group(1)
        try:
            rust = subprocess.run([str(crate / 'target' / 'release' / package)], cwd=run,
                                  capture_output=True, **UTF8, stdin=subprocess.DEVNULL,
                                  timeout=RUST_TIMEOUT)
        except subprocess.TimeoutExpired:
            return finish('rust-timeout', 'the translation ran past the time limit')

        # Java: the source launcher compiles and runs the program; it passed javac above.
        try:
            java = subprocess.run(JAVA + ['Prog.java'], cwd=run, capture_output=True,
                                  stdin=subprocess.DEVNULL, **UTF8, timeout=JAVA_TIMEOUT)
        except subprocess.TimeoutExpired:
            return finish('java-timeout', 'Java ran past the time limit')
        if (rust.stdout, rust.returncode, exception(rust.stderr)) == \
                (java.stdout, java.returncode, exception(java.stderr)):
            return finish('match', '')
        (run / 'java.out').write_text(java.stdout, encoding='utf-8')
        (run / 'rust.out').write_text(rust.stdout, encoding='utf-8')
        return finish('mismatch', describe(java, rust), key='')


def report(results):
    counts = Counter(r['status'] for r in results)
    total = len(results)
    print(f'\n{total} programs: ' + ', '.join(f'{s} {counts[s]}' for s in STATUSES if counts[s]))
    rejected = Counter(r['key'] for r in results if r['status'] == 'rejected')
    if rejected:
        print('\nrejections (count, message):')
        for message, n in rejected.most_common():
            print(f'  {n:4}  {message}')
    for status in FAILURES + ('invalid', 'java-timeout', 'harness-error'):
        listed = [r for r in results if r['status'] == status]
        for r in listed[:15]:
            print(f'  {status:16} {r["name"]:>8}  {r["detail"]}')
        if len(listed) > 15:
            print(f'  {status:16} ... and {len(listed) - 15} more')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--count', type=int, default=200, help='number of seeds')
    ap.add_argument('--start', type=int, default=1, help='first seed')
    ap.add_argument('--size', type=int, default=30, help='statements in main, per program')
    ap.add_argument('--jobs', type=int, default=4, help='programs in parallel')
    ap.add_argument('--strings', action='store_true',
                    help='also generate String, StringBuilder, String.format and Arrays code')
    ap.add_argument('--arrays', action='store_true',
                    help='also generate arrays of every depth, aliases, moves, rows and loops')
    ap.add_argument('--objects', action='store_true',
                    help='also generate class trees, interfaces, Object values and generics')
    ap.add_argument('--no-build', action='store_true', help='skip cargo build of the translator')
    args = ap.parse_args()

    if not args.no_build:
        build_translator()
    WORK.mkdir(parents=True, exist_ok=True)
    runner = Runner(args.jobs)
    items = [(f's{seed}', fuzz.generate(seed, args.size, strings=args.strings, arrays=args.arrays,
                                        objects=args.objects))
             for seed in range(args.start, args.start + args.count)]
    results = []
    for i, result in enumerate(runner.classify_many(items), 1):
        results.append(result)
        if i % 20 == 0:
            print(f'{i}/{len(items)} done', file=sys.stderr)
    with open(WORK / 'results.jsonl', 'w') as f:
        for seed, r in zip(range(args.start, args.start + args.count), results):
            f.write(json.dumps({'seed': seed, **r}) + '\n')
    report(results)


if __name__ == '__main__':
    main()
