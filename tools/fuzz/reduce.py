#!/usr/bin/env python3
"""Shrinks a program that fails in the translator, keeping the same failure as it shrinks.

    python3 tools/fuzz/reduce.py target/fuzz/runs/s17/Prog.java [--out Reduced.java]
                                 [--jobs 4] [--max-tests 400]

A failure's class is its status and key: mismatch, rustc (key: the error code, or the first 60
characters of the message), translator-crash, rust-timeout or rejected (key: the message). A
candidate is kept only when it has the same class as the original. javac runs inside the
translator, so a candidate javac rejects is `invalid` and is never kept; Rust is built only for
programs the translator accepted, and Java runs only when the Rust output is needed.

Two passes, repeated until neither shrinks the program:
  1. delete a statement or member: one line, or a block from a line ending in `{` to its
     closing `}`. Lines indented 4 spaces or more are candidates, so the class header stays.
     The pass goes bottom up, so each range is tried once and an accepted deletion does not
     move the ranges above it.
  2. simplify a parenthesised group: replace it with 0, 1, true or false, or with one operand
     of its top-level binary operator. Groups that follow a name (calls, `m0(`) are skipped:
     replacing their argument list would change the program's shape, not just simplify it.

Candidates run in parallel batches (`--jobs`); the first kept candidate in order is accepted.
Results are cached by text, so no candidate runs twice. `--max-tests` caps the candidates run.
"""

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run as fuzzrun  # noqa: E402

MEMBER_INDENT = 4  # class members and the body of each method
MAIN = 'public static void main('  # a candidate without it has no Java run to compare
REDUCIBLE = ('mismatch', 'rustc', 'translator-crash', 'rust-timeout', 'rejected')
BINARY_OPS = ['&&', '||', '<<', '>>>', '>>', '<=', '>=', '==', '!=', '+', '-', '*', '/', '%',
              '&', '|', '^', '<', '>']
REPLACEMENTS = ['0', '1', 'true', 'false']


def units(lines):
    """Statement ranges (first, last line index) that a deletion may remove."""
    out = []
    for i, line in enumerate(lines):
        stripped = line.lstrip()
        indent = len(line) - len(stripped)
        if indent < MEMBER_INDENT or not stripped or stripped.startswith('}'):
            continue
        end = i
        if stripped.endswith('{'):
            balance = 0
            for j in range(i, len(lines)):
                balance += lines[j].count('{') - lines[j].count('}')
                if balance == 0:
                    end = j
                    break
        out.append((i, end))
    return out


def families(lines):
    """The units inside each unit (or outside all of them), as lists of sibling units in order."""
    us = units(lines)
    kids = {}
    for k, (s, e) in enumerate(us):
        containing = [j for j, (a, b) in enumerate(us) if a < s and e <= b]
        parent = min(containing, key=lambda j: us[j][1] - us[j][0], default=None)
        kids.setdefault(parent, []).append(us[k])
    return list(kids.values())


def runs(lines, chunk):
    """Line ranges (first, last) of `chunk` consecutive siblings, the bottom ones first."""
    out = [(fam[i][0], fam[i + chunk - 1][1])
           for fam in families(lines) for i in range(len(fam) - chunk + 1)]
    return sorted(out, reverse=True)


def groups(line):
    """Balanced parenthesised spans (start, end) of a line, the longest first."""
    stack, out = [], []
    for k, ch in enumerate(line):
        if ch == '(':
            stack.append(k)
        elif ch == ')' and stack:
            start = stack.pop()
            before = line[start - 1] if start else ' '
            if not (before.isalnum() or before in '_]'):
                out.append((start, k))
    return sorted(out, key=lambda g: g[0] - g[1])


def operands(inner):
    """The two operands around the first top-level binary operator of `inner`, if any."""
    depth = 0
    for k, ch in enumerate(inner):
        if ch in '([':
            depth += 1
        elif ch in ')]':
            depth -= 1
        elif depth == 0 and ch == ' ':
            for op in BINARY_OPS:
                if inner.startswith(f' {op} ', k):
                    return inner[:k], inner[k + len(op) + 2:]
    return None


def replacements(line, start, end):
    """What the group at (start, end) of `line` may become: constants, then its operands."""
    split = operands(line[start + 1:end]) or ()
    return REPLACEMENTS + [part for part in split if part]


def text_of(lines):
    return '\n'.join(lines) + '\n'


class Reducer:
    def __init__(self, runner, signature, jobs, max_tests):
        self.runner = runner
        self.signature = signature
        self.jobs = jobs
        self.max_tests = max_tests
        self.tests = 0
        self.seen = {}  # text -> whether it keeps the failure

    def candidate(self, lines, size):
        """The text of `lines` as a candidate, or None when it cannot be a smaller program."""
        text = text_of(lines)
        return text if len(text) < size and MAIN in text else None

    def first_kept(self, texts):
        """Index of the first text that keeps the failure, or None. None entries are skipped.

        Texts run in parallel batches of `jobs`; the batch that holds the answer is always run
        whole, so the result depends only on the order of the texts.
        """
        for start in range(0, len(texts), self.jobs):
            if self.tests >= self.max_tests:
                break
            chunk = texts[start:start + self.jobs]
            fresh = [t for t in dict.fromkeys(chunk) if t is not None and t not in self.seen]
            fresh = fresh[:self.max_tests - self.tests]
            self.tests += len(fresh)
            items = [(f'reduce{i}', t) for i, t in enumerate(fresh)]
            for t, result in zip(fresh, self.runner.classify_many(items)):
                self.seen[t] = (result['status'], result['key']) == self.signature
            for k, t in enumerate(chunk):
                if t is not None and self.seen.get(t):
                    return start + k
        return None

    def delete_pass(self, lines):
        """Deletes runs of siblings, largest runs first (halving, as delta debugging does)."""
        chunk = 1
        while 2 * chunk <= max((len(kids) for kids in families(lines)), default=0):
            chunk *= 2
        while chunk >= 1 and self.tests < self.max_tests:
            lines = self.delete_runs(lines, chunk)
            chunk //= 2
        return lines

    def delete_runs(self, lines, chunk):
        """Deletes runs of `chunk` siblings from the bottom up; each run is tried once."""
        size = len(text_of(lines))
        pos = len(lines)  # only runs that start above `pos` are still untried
        while self.tests < self.max_tests:
            ranges = [r for r in runs(lines, chunk) if r[0] < pos][:self.jobs]
            if not ranges:
                break
            texts = [self.candidate(lines[:s] + lines[e + 1:], size) for s, e in ranges]
            k = self.first_kept(texts)
            if k is None:
                pos = ranges[-1][0]
            else:
                start, end = ranges[k]
                lines = lines[:start] + lines[end + 1:]
                size = len(text_of(lines))
                pos = start
        return lines

    def simplify_pass(self, lines):
        """Replaces parenthesised groups by simpler text, top to bottom; returns the lines kept."""
        i = 0
        while i < len(lines) and self.tests < self.max_tests:
            changed = self.simplify_line(lines, i)
            if changed is None:
                i += 1
            else:
                lines = changed  # the same line again: its groups have changed
        return lines

    def simplify_line(self, lines, i):
        """The lines with one group of line `i` simplified, or None when none can be."""
        line = lines[i]
        if len(line) - len(line.lstrip()) < MEMBER_INDENT:
            return None
        size = len(text_of(lines))
        for start, end in groups(line):
            variants = [line[:start] + '(' + r + ')' + line[end + 1:]
                        for r in replacements(line, start, end)]
            texts = [self.candidate(lines[:i] + [v] + lines[i + 1:], size) for v in variants]
            k = self.first_kept(texts)
            if k is not None:
                return lines[:i] + [variants[k]] + lines[i + 1:]
        return None

    def reduce(self, text):
        lines = text.rstrip('\n').split('\n')
        while self.tests < self.max_tests:
            new = self.simplify_pass(self.delete_pass(lines))
            if new == lines:
                break
            lines = new
            print(f'  {len(lines)} lines, {self.tests} tests', file=sys.stderr)
        return text_of(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('program', type=Path, help='the Java program that fails')
    ap.add_argument('--out', type=Path, help='where to write the reduced program')
    ap.add_argument('--jobs', type=int, default=4, help='candidates run in parallel')
    ap.add_argument('--max-tests', type=int, default=400, help='candidates to run at most')
    ap.add_argument('--no-build', action='store_true', help='skip cargo build of the translator')
    args = ap.parse_args()

    started = time.monotonic()
    text = args.program.read_text(encoding='utf-8')
    if not args.no_build:
        fuzzrun.build_translator()
    runner = fuzzrun.Runner(args.jobs, keep=False)
    original = next(runner.classify_many([('original', text)]))
    signature = (original['status'], original['key'])
    print(f'original: {original["status"]} {original["detail"]}', file=sys.stderr)
    if original['status'] not in REDUCIBLE:
        sys.exit(f'{args.program} does not fail in a way that reduction keeps: {original["status"]}')

    reducer = Reducer(runner, signature, args.jobs, args.max_tests)
    reduced = reducer.reduce(text)
    out = args.out or args.program.with_name('Reduced.java')
    out.write_text(reduced, encoding='utf-8')
    seconds = time.monotonic() - started
    print(f'reduced to {len(reduced.splitlines())} lines in {reducer.tests} tests, '
          f'{seconds:.0f} s: {out}')


if __name__ == '__main__':
    main()
