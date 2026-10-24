#!/usr/bin/env python3
"""Puts a jar's sources together from several decompiles, file by file (see
docs/minecraft-server.md, and the best-of and library-best-of steps of minecraft-server-survey.sh).

  decompile-best-of.py DIR OUT CLASSPATH MAX_COMPILES SETUP... [-- SLOT...]

DIR holds one tree per setup (DIR/<setup>), all-files.txt (the first setup's files, relative) and
<first setup>-javac.txt (javac's log on the first setup's whole tree). The first setup is the
base: a file is taken from another setup only where the whole tree then has fewer javac errors.
A setup without a tree (CFR is not run where the base has no errors) is skipped.

1. For each other setup, the base tree with that setup's version of every file that has errors
   in the base. A file is a candidate from the setup where it has the fewest errors, if fewer
   than in the base.
2. The base tree with every candidate, compiled again and again: each round drops the swapped
   files that do not have fewer errors than in the base, and those whose class is named in a
   file that is not swapped and has more errors than in the base. The round with the fewest
   errors over the whole tree is kept, if it has fewer than the base.

Every compile is of the whole tree, against the other jars only (CLASSPATH), at most
MAX_COMPILES of them, counting the base's. Writes OUT (the tree kept) and DIR/chosen.txt
("setup<TAB>file<TAB>base errors<TAB>errors"), and prints the counts.
"""
import collections
import os
import re
import shutil
import subprocess
import sys
import tempfile

args = sys.argv[1:]
slot = []
if "--" in args:
    args, slot = args[: args.index("--")], args[args.index("--") + 1 :]
dir, out, classpath, max_compiles, *setups = args
max_compiles = int(max_compiles)
base_setup = setups[0]
compiles = 1  # the base's, already done


def counts_of(log):
    counts = collections.Counter()
    with open(log, errors="replace") as f:
        for line in f:
            if ": error: " in line:
                counts[line.split(":", 1)[0]] += 1
    return counts


def build(choice, tree):
    shutil.rmtree(tree, ignore_errors=True)
    shutil.copytree(f"{dir}/{base_setup}", tree)
    for file, setup in choice.items():
        shutil.copy(f"{dir}/{setup}/{file}", f"{tree}/{file}")


def compile_with(choice, name):
    """The whole tree with the files of CHOICE swapped in: javac's errors per file."""
    global compiles
    compiles += 1
    tree = f"{dir}/trial"
    build(choice, tree)
    classes = tempfile.mkdtemp()
    log = f"{dir}/{name}-javac.txt"
    with open(log, "w") as f:
        subprocess.run(
            slot + ["javac", "-J-Xmx3g", "-proc:none", "-nowarn", "-Xmaxerrs", "100000",
                    "-encoding", "UTF-8", "--should-stop=ifError=FLOW", "-d", classes,
                    "-cp", classpath, f"@{dir}/all-files.txt"],
            cwd=tree, stdout=f, stderr=subprocess.STDOUT)
    shutil.rmtree(classes)
    shutil.rmtree(tree)
    return counts_of(log)


base = counts_of(f"{dir}/{base_setup}-javac.txt")
base_total = sum(base.values())
print(f"{base_setup} alone: {base_total} errors in {len(base)} files")

# 1. Each other setup's version of every file with errors in the base.
best = {}  # file -> (errors, setup)
for setup in setups[1:]:
    if compiles >= max_compiles:
        break
    swapped = {f: setup for f in base if os.path.isfile(f"{dir}/{setup}/{f}")}
    if not swapped:
        continue
    counts = compile_with(swapped, setup)
    print(f"{setup} in place of {len(swapped)} files: {sum(counts.values())} errors in the tree")
    for f in swapped:
        if counts[f] < base[f] and counts[f] < best.get(f, (base[f],))[0]:
            best[f] = (counts[f], setup)
choice = {f: s for f, (_, s) in best.items()}

# 2. All candidates together, dropping the ones that do not help.
kept, kept_total, kept_counts = {}, base_total, base
round_number = 0
while choice and compiles < max_compiles:
    round_number += 1
    counts = compile_with(choice, f"round-{round_number}")
    total = sum(counts.values())
    print(f"round {round_number}: {len(choice)} files swapped, {total} errors in the tree")
    if total < kept_total:
        kept, kept_total, kept_counts = dict(choice), total, counts
    drop = {f for f in choice if counts[f] >= base[f]}
    names = {f: re.compile(r"\b" + re.escape(os.path.basename(f)[: -len(".java")]) + r"\b") for f in choice}
    for victim in counts:
        if victim in choice or counts[victim] <= base[victim]:
            continue
        with open(f"{dir}/{base_setup}/{victim}", errors="replace") as text:
            source = text.read()
        drop |= {f for f in choice if names[f].search(source)}
    if not drop:
        break
    choice = {f: s for f, s in choice.items() if f not in drop}

build(kept, out)
with open(f"{dir}/chosen.txt", "w") as f:
    for file, setup in sorted(kept.items()):
        print(f"{setup}\t{file}\t{base[file]}\t{kept_counts[file]}", file=f)
by_setup = collections.Counter(kept.values())
print(f"kept: {kept_total} errors in {len(kept_counts)} files, with "
      + ", ".join(f"{by_setup[s]} files from {s}" for s in setups[1:])
      + f" ({compiles} compiles)")
