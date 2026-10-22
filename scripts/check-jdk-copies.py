#!/usr/bin/env python3
"""Fails if the translator's Java files hold code copied from the JDK.

Usage: scripts/check-jdk-copies.py JAVA_BASE_SOURCES [--verbose]

The translator's patches of JDK classes (crates/translator/java/patches) and its own classes
(crates/translator/java/overlay) must hold only code written for this repository (see
docs/jdk-overlays.md). For each such file, this compares its lines with those of the JDK's file of
the same path, and fails when:

- an overlay file has the path of a JDK file: a change to a JDK class is a patch, so overlay/
  holds only classes the JDK does not have;
- a file shares more than MAX_SHARED lines with the JDK's file of the same path.

Lines that say nothing on their own are not counted: blank lines, package and import lines,
lines of only brackets and punctuation, the bare lines of comments (`/**`, `*`, `*/`), lines
shorter than MIN_LENGTH characters (`return true;`, `} else {`), and in a patch the lines that name
what it patches: the class's line, and the declaration that follows a marker annotation
(`@Replace`, `@Remove`, ...), which names the JDK's member it changes.
A line that is there several times counts once.
"""

import os
import re
import sys

MAX_SHARED = 12
MIN_LENGTH = 16
MARKER = re.compile(r"^@(Replace|Add|Remove|Edit|Patch)\b")
TRIVIAL = re.compile(r"^[\s{}()\[\];,.*/]*$")
CLASS = re.compile(r"^((public|protected|private|abstract|final|sealed|static)\s+)*(class|interface|enum|record)\s")

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "translator", "java")


def counted(lines, skip_declarations):
    """The lines of a file that count, stripped, without each marker's declaration."""
    out = []
    in_declaration = False
    for line in lines:
        text = line.strip()
        if skip_declarations and CLASS.match(text):
            # The class that a patch names.
            continue
        if skip_declarations and MARKER.match(text):
            in_declaration = True
            continue
        if in_declaration:
            if text.startswith("@"):
                continue
            # The declaration ends at its first line that opens the body or ends it.
            if text.endswith("{") or text.endswith(";") or text.endswith("}"):
                in_declaration = False
            continue
        if not text or text.startswith(("import ", "package ")) or TRIVIAL.match(text):
            continue
        if len(text) < MIN_LENGTH:
            continue
        out.append(text)
    return out


def read(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read().splitlines()


def main():
    args = [a for a in sys.argv[1:] if a != "--verbose"]
    verbose = "--verbose" in sys.argv[1:]
    if len(args) != 1 or not os.path.isdir(args[0]):
        print(__doc__.strip().splitlines()[2], file=sys.stderr)
        return 2
    jdk = args[0]
    failures = []
    for kind in ("patches", "overlay"):
        base = os.path.join(ROOT, kind)
        for directory, _, names in sorted(os.walk(base)):
            for name in sorted(names):
                if not name.endswith(".java"):
                    continue
                path = os.path.join(directory, name)
                relative = os.path.relpath(path, base)
                theirs = os.path.join(jdk, relative)
                if not os.path.isfile(theirs):
                    if kind == "patches":
                        failures.append(f"patches/{relative}: the JDK has no file {relative} to patch")
                    continue
                if kind == "overlay":
                    failures.append(f"overlay/{relative}: the JDK has this class; change it with a patch in patches/")
                jdk_lines = set(counted(read(theirs), False))
                shared = sorted(set(counted(read(path), kind == "patches")) & jdk_lines)
                if verbose or len(shared) > MAX_SHARED:
                    print(f"{kind}/{relative}: {len(shared)} lines shared with the JDK's file")
                    for line in shared:
                        print(f"    {line}")
                if len(shared) > MAX_SHARED:
                    failures.append(
                        f"{kind}/{relative}: {len(shared)} lines are the JDK's (at most {MAX_SHARED}): "
                        "keep only the changes, with @Edit for a few changed lines"
                    )
    for failure in failures:
        print(failure)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
