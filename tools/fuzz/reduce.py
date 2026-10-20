#!/usr/bin/env python3
"""Shrinks a program that fails in the translator, keeping the same failure as it shrinks.

    python3 tools/fuzz/reduce.py target/fuzz/runs/s17/Prog.java [--out Reduced.java]
                                 [--jobs 4] [--max-tests 400]

A candidate is kept when it fails the same way as the original: the same status (mismatch,
rustc, translator-crash or rust-timeout) and the same key (for rustc, the error code). A
candidate that javac rejects, or that is fixed or fails differently, is discarded.

Two kinds of candidate, tried until neither shrinks the program:
  1. delete a statement or member: one line, or a block from a line ending in `{` to its
     closing `}`. Lines indented 4 spaces or more are candidates, so the class header stays.
  2. simplify a parenthesised group: replace it with 0, 1, true or false, or with one operand
     of its top-level binary operator. Groups that follow a name (calls, `m0(`) are skipped:
     replacing their argument list would change the program's shape, not just simplify it.

Candidates are run in parallel batches; the first one in order that keeps the failure is
accepted. Results are cached by text, so no candidate is run twice.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run as fuzzrun  # noqa: E402

MEMBER_INDENT = 4  # class members and the body of each method
MAIN = 'public static void main('  # a candidate without it has no Java run to compare
FAILURES = ('mismatch', 'rustc', 'translator-crash', 'rust-timeout')
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


def deletions(lines):
    """Candidate texts, each with one statement range removed; the largest ranges first."""
    for start, end in sorted(units(lines), key=lambda u: u[0] - u[1]):
        yield '\n'.join(lines[:start] + lines[end + 1:]) + '\n'


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


def simplifications(lines):
    """Candidate texts, each with one parenthesised group replaced by something simpler."""
    for i, line in enumerate(lines):
        if len(line) - len(line.lstrip()) < MEMBER_INDENT:
            continue
        for start, end in groups(line):
            replacements = list(REPLACEMENTS)
            split = operands(line[start + 1:end])
            if split:
                replacements += [part for part in split if part]
            for r in replacements:
                changed = line[:start] + '(' + r + ')' + line[end + 1:]
                yield '\n'.join(lines[:i] + [changed] + lines[i + 1:]) + '\n'


class Reducer:
    def __init__(self, runner, signature, jobs, max_tests):
        self.runner = runner
        self.signature = signature
        self.jobs = jobs
        self.max_tests = max_tests
        self.tests = 0
        self.seen = {}  # text -> whether it keeps the failure

    def first_kept(self, candidates, limit):
        """Runs the untested candidates in batches; returns the first one that keeps the failure.

        Only candidates shorter than `limit` characters are tried, so every accepted candidate
        is strictly smaller than the one it replaces and the reduction must end.
        """
        batch = []
        for text in candidates:
            if self.tests >= self.max_tests:
                break
            if len(text) >= limit or MAIN not in text:
                continue
            if text in self.seen:
                if self.seen[text]:
                    return text
                continue
            if text not in batch:
                batch.append(text)
            if len(batch) == self.jobs:
                kept = self.test(batch)
                batch = []
                if kept:
                    return kept
        return self.test(batch) if batch else None

    def test(self, batch):
        self.tests += len(batch)
        items = [(f'reduce{i}', text) for i, text in enumerate(batch)]
        kept = None
        for text, result in zip(batch, self.runner.classify_many(items)):
            self.seen[text] = (result['status'], result['key']) == self.signature
            if self.seen[text] and kept is None:
                kept = text
        return kept

    def reduce(self, text):
        lines = text.rstrip('\n').split('\n')
        progress = True
        while progress and self.tests < self.max_tests:
            progress = False
            for generate in (deletions, simplifications):
                while self.tests < self.max_tests:
                    current = '\n'.join(lines) + '\n'
                    kept = self.first_kept(generate(lines), len(current))
                    if kept is None:
                        break
                    lines = kept.rstrip('\n').split('\n')
                    progress = True
                    print(f'  {len(lines)} lines, {self.tests} tests', file=sys.stderr)
        return '\n'.join(lines) + '\n'


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('program', type=Path, help='the Java program that fails')
    ap.add_argument('--out', type=Path, help='where to write the reduced program')
    ap.add_argument('--jobs', type=int, default=4, help='candidates run in parallel')
    ap.add_argument('--max-tests', type=int, default=400, help='candidates to run at most')
    ap.add_argument('--no-build', action='store_true', help='skip cargo build of the translator')
    args = ap.parse_args()

    text = args.program.read_text()
    if not args.no_build:
        fuzzrun.build_translator()
    runner = fuzzrun.Runner(args.jobs, keep=False)
    original = next(runner.classify_many([('original', text)]))
    signature = (original['status'], original['key'])
    print(f'original: {original["status"]} {original["detail"]}', file=sys.stderr)
    if original['status'] not in FAILURES:
        sys.exit(f'{args.program} does not fail in a way that reduction keeps: {original["status"]}')

    reducer = Reducer(runner, signature, args.jobs, args.max_tests)
    reduced = reducer.reduce(text)
    out = args.out or args.program.with_name('Reduced.java')
    out.write_text(reduced)
    print(f'reduced to {len(reduced.splitlines())} lines in {reducer.tests} tests: {out}')


if __name__ == '__main__':
    main()
