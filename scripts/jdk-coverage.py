#!/usr/bin/env python3
"""Regenerates docs/jdk-coverage.md: how much of the JDK's code that the target programs reach the
translator translates, measured by its census (`--census-json`).

Usage: python3 scripts/jdk-coverage.py JDK_JAVA_BASE_ROOT [--no-run] [--per-target FILE]

Builds the translator (release), runs the census of each program of targets/ and of
targets/driver/JdkApiDriver.java, translated whole with the JDK's sources, one after another (each
takes a few seconds and about 1.5 GB), and writes the report. The censuses go to
target/jdk-coverage/<Name>.json; `--no-run` reuses them. `--per-target FILE` also writes every
program's distinct rejections, grouped by message, to FILE.
"""

import datetime
import json
import os
import re
import shutil
import subprocess
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORK = os.path.join(ROOT, "target", "jdk-coverage")
DRIVER = "JdkApiDriver"
TOP = 15


def programs():
    names = sorted(f[:-5] for f in os.listdir(os.path.join(ROOT, "targets")) if f.endswith(".java"))
    return [(n, os.path.join(ROOT, "targets", n + ".java")) for n in names] + [
        (DRIVER, os.path.join(ROOT, "targets", "driver", DRIVER + ".java"))
    ]


def run(jdk):
    target_dir = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
    subprocess.run(["cargo", "build", "--release", "-q", "-p", "java-to-rust"], cwd=ROOT, check=True)
    translator = os.path.join(target_dir, "release", "java-to-rust")
    os.makedirs(WORK, exist_ok=True)
    for name, java in programs():
        # A listed file next to a module's source root is rejected by javac, so each program is a
        # source directory of its own, as targets/run.sh gives it.
        src = os.path.join(WORK, "src-" + name)
        os.makedirs(src, exist_ok=True)
        shutil.copy(java, src)
        out = os.path.join(WORK, name + ".json")
        result = subprocess.run(
            [translator, "--main", name, "--source", src, "--library", jdk, "--census-json", out],
            capture_output=True, text=True)
        status = "ok" if result.returncode == 0 else "failed: " + result.stderr.strip().splitlines()[-1]
        print(f"{name}: {status}", file=sys.stderr)


def load():
    censuses = {}
    for name, _ in programs():
        path = os.path.join(WORK, name + ".json")
        if os.path.exists(path):
            with open(path) as f:
                censuses[name] = json.load(f)
    return censuses


def key(piece):
    return (piece["class"], piece["kind"], piece["name"], piece["location"])


def normalize(message):
    """The message with the names in backquotes left out, as the census groups them."""
    return re.sub(r"`([^`]*)`", lambda m: "`…`" if re.search(r"[.<\[(]", m.group(1)) else m.group(0), message)


def percent(part, whole):
    return f"{100 * part / whole:.1f}%" if whole else "-"


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True).stdout.strip()


def share(pieces, methods_only=False):
    """Pieces (or methods and constructors) that translate, of those reached, as `n / m (p)`."""
    chosen = [p for p in pieces.values() if not methods_only or p["kind"] in ("method", "constructor")]
    good = sum(1 for p in chosen if not p["errors"])
    return f"{good} / {len(chosen)} ({percent(good, len(chosen))})", good, len(chosen)


def union(censuses, names, source):
    """The pieces of one source that the programs reach, each with the errors any of them finds."""
    pieces = {}
    for name in names:
        for p in censuses[name]["pieces"]:
            if p["source"] != source:
                continue
            entry = pieces.setdefault(key(p), {"kind": p["kind"], "errors": set(), "programs": set()})
            entry["errors"].update(e["message"] for e in p["errors"])
            entry["programs"].add(name)
    return pieces


def blockers(censuses, names, source=None):
    """Per normalized message: the distinct pieces it blocks, the programs, and the exact messages."""
    rows = defaultdict(lambda: {"pieces": set(), "programs": set(), "exact": defaultdict(set)})
    for name in names:
        census = censuses[name]
        for p in census["pieces"]:
            if source and p["source"] != source:
                continue
            for e in p["errors"]:
                row = rows[normalize(e["message"])]
                row["pieces"].add(key(p))
                row["programs"].add(name)
                row["exact"][e["message"]].add(key(p))
    return rows


def per_target(censuses, names):
    """Each program's distinct rejections, grouped by normalized message, with the pieces each
    blocks (program and library code together), after their union, by the programs each is in."""
    lines = ["### All targets (the driver left out)\n", "| Programs | Pieces | Rejection |", "|---:|---:|---|"]
    rows = blockers(censuses, [n for n in names if n != DRIVER])
    for message, row in sorted(rows.items(), key=lambda r: (-len(r[1]["programs"]), -len(r[1]["pieces"]), r[0])):
        lines.append(f"| {len(row['programs'])} | {len(row['pieces'])} | {escape(message)} |")
    lines.append("")
    for name in names:
        census = censuses[name]
        totals = census["totals"]
        status = "complete" if census["complete"] else f"incomplete, stopped at {census['fatal']['place']}: {census['fatal']['message']}"
        lines.append(f"### {name}\n")
        lines.append(
            f"Program code: {totals['program']['translate']} / {totals['program']['pieces']} pieces translate; "
            f"library code: {totals['library']['translate']} / {totals['library']['pieces']}; {status}.\n")
        rows = blockers(censuses, [name])
        if not rows:
            lines.append("No rejections.\n")
            continue
        lines.append("| Pieces | Rejection |")
        lines.append("|---:|---|")
        for message, row in sorted(rows.items(), key=lambda r: (-len(r[1]["pieces"]), r[0])):
            lines.append(f"| {len(row['pieces'])} | {escape(message)} |")
        lines.append("")
    return "\n".join(lines) + "\n"


def escape(text):
    return text.replace("|", "\\|")


def top_table(rows, order, with_examples=True):
    lines = ["| Rejection | Pieces | Programs | Most common forms |", "|---|---:|---:|---|"]
    for message, row in sorted(rows.items(), key=order)[:TOP]:
        forms = sorted(row["exact"].items(), key=lambda f: (-len(f[1]), f[0]))
        forms = "; ".join(f"{escape(m)} ({len(p)})" for m, p in forms[:3]) if len(forms) > 1 else ""
        lines.append(f"| {escape(message)} | {len(row['pieces'])} | {len(row['programs'])} | {forms} |")
    return "\n".join(lines)


def natives(censuses, names):
    missing = defaultdict(set)
    reached = defaultdict(set)
    for name in names:
        for n in censuses[name]["natives"]:
            reached[n["method"]].add(name)
            if not n["runtime"]:
                missing[n["method"]].add(name)
    return reached, missing


def report(censuses, jdk):
    targets = [n for n, _ in programs() if n != DRIVER and n in censuses]
    commit = git("rev-parse", "--short", "HEAD")
    # The last commit of main that the measured one has: itself once main has it.
    base = git("merge-base", "HEAD", "main")
    on_main = "on main" if base == git("rev-parse", "HEAD") else f"on main at `{base[:7]}`"
    date = datetime.date.today().isoformat()
    incomplete = [n for n in censuses if not censuses[n]["complete"]]
    lib_targets = union(censuses, targets, "library")
    lib_driver = union(censuses, [DRIVER], "library") if DRIVER in censuses else {}
    own_targets = union(censuses, targets, "program")
    out = []
    w = out.append
    w("# JDK coverage\n")
    w(f"How much of the JDK's code that typical programs reach the translator translates today. "
      f"Measured on {date} at commit `{commit}` ({on_main}) by `scripts/jdk-coverage.py`, which regenerates this "
      f"file, over the {len(targets)} programs of `targets/` and the driver "
      f"`targets/driver/{DRIVER}.java`, each translated whole with the sources of `java.base`.\n")
    missing = [n for n, _ in programs() if n not in censuses]
    if missing:
        w(f"No census of {', '.join(missing)}: it did not finish (see Limits).\n")
    w("## Method\n")
    w("`java-to-rust --census-json` (`program/census.rs`) runs the translator's real pipeline (lowering, "
      "the analyses and the generator) over everything reachable from `main`, without stopping at the "
      "first rejection. Lowering records each rejected method, constructor or lambda and lowers a stub "
      "in its place, with its signature and a body that never runs; a rejected field initializer or "
      "initializer block is left out. The errors of an analysis or of the generator stub the methods "
      "at their positions, and they run again (the generator and the analysis of exceptions report "
      "every error they find at once, the other analyses their first). So every rejection counted is "
      "one that the translation would report once the code before it translated. A *piece* is a "
      "reachable method, constructor, field "
      "initializer or initializer block, as `--survey` counts them, with the lambdas, local and "
      "anonymous classes it declares; it translates when the census records no rejection in it. "
      "Messages are grouped with the names in backquotes left out (`type \\`…\\` not supported yet`).\n")
    w("Limits:\n")
    w("- A piece counts as translating when the census records no rejection in it. It may still call "
      "a piece that does not, and its translation was neither compiled nor run.")
    w("- A piece is stubbed at its first rejection in each phase, so a piece with several blockers "
      "may show only some of them; the counts of pieces blocked are exact, the lists of blockers per "
      "piece are a lower bound.")
    w("- An error of an analysis or of the generator is at a line and column of the file of the code "
      "it is in, and the census stubs the code at that position in that file alone. One in code that "
      "lowering provides has no file: the census stubs the code at its position in every file, so on "
      "a collision it also hides the code of an innocent method from the later phases.")
    w("- A rejection that no stub can take away ends the census early, and so does a panic of the "
      "translator outside the generator; the numbers then count only what was checked before the "
      "stop. A rejection of a class's declaration, or in code that lowering provides, belongs to no "
      "piece; it is listed in the census as rejected elsewhere.")
    w("- The analyses run again after each round of stubs. Most programs take seconds, but those that "
      "reach `Scanner` (regular expressions, `java.time`; about 9000 pieces, where one run of the "
      "analyses takes minutes) take most of an hour or more.")
    w("- " + (f"Incomplete censuses: {', '.join(sorted(incomplete))} (see the per-program table)."
              if incomplete else "Every census here is complete."))
    w("")
    w("## Headline\n")
    s_all, _, _ = share(lib_targets)
    m_all, _, _ = share(lib_targets, True)
    w(f"- Library code reached by the {len(targets)} targets (union): {s_all} pieces translate; "
      f"methods and constructors alone: {m_all}.")
    if lib_driver:
        s_d, _, _ = share(lib_driver)
        m_d, _, _ = share(lib_driver, True)
        w(f"- Library code reached by the driver: {s_d} pieces translate; methods and constructors: {m_d}.")
    s_own, _, _ = share(own_targets)
    w(f"- The targets' own code: {s_own} pieces translate.")
    clean = [n for n in targets if censuses[n]["complete"] and not any(p["errors"] for p in censuses[n]["pieces"])]
    w(f"- Programs with no rejection at all: {len(clean)} of {len(targets)}"
      + (f" ({', '.join(clean)})." if clean else "."))
    w("")
    w("## Per program\n")
    w("| Program | Own pieces translate | Library pieces translate | Distinct rejections | Complete |")
    w("|---|---:|---:|---:|---|")
    for name in targets + ([DRIVER] if DRIVER in censuses else []):
        c = censuses[name]
        t = c["totals"]
        distinct = len(blockers(censuses, [name]))
        w(f"| {name} | {t['program']['translate']} / {t['program']['pieces']} | "
          f"{t['library']['translate']} / {t['library']['pieces']} ({percent(t['library']['translate'], t['library']['pieces'])}) | "
          f"{distinct} | {'yes' if c['complete'] else 'no: ' + escape(c['fatal']['message']) + ' (' + c['fatal']['place'] + ')'} |")
    w("")
    w("## Rejections per program\n")
    w(f"Each program's rejections, grouped by message, with the pieces each blocks (up to {TOP}; "
      f"the programs with none are left out).\n")
    for name in targets + ([DRIVER] if DRIVER in censuses else []):
        rows = blockers(censuses, [name])
        if not rows:
            continue
        w(f"### {name}\n")
        w("| Pieces | Rejection |")
        w("|---:|---|")
        for message, row in sorted(rows.items(), key=lambda r: (-len(r[1]["pieces"]), r[0]))[:TOP]:
            w(f"| {len(row['pieces'])} | {escape(message)} |")
        if len(rows) > TOP:
            w(f"| | and {len(rows) - TOP} more |")
        w("")
    rows = blockers(censuses, targets)
    w(f"## Top {TOP} rejections over the targets, by pieces they block\n")
    w("Pieces are counted once however many programs reach them; program and library code together.\n")
    w(top_table(rows, lambda r: (-len(r[1]["pieces"]), -len(r[1]["programs"]), r[0])))
    w("")
    w(f"## Top {TOP} rejections over the targets, by programs they appear in\n")
    w(top_table(rows, lambda r: (-len(r[1]["programs"]), -len(r[1]["pieces"]), r[0])))
    w("")
    if DRIVER in censuses:
        w(f"## Top {TOP} rejections in the driver, by pieces they block\n")
        w(top_table(blockers(censuses, [DRIVER]), lambda r: (-len(r[1]["pieces"]), r[0])))
        w("")
    reached, missing = natives(censuses, targets + ([DRIVER] if DRIVER in censuses else []))
    w("## Natives\n")
    w(f"The programs reach {len(reached)} native methods; the runtime has "
      f"{len(reached) - len(missing)} of them. Missing, with the number of programs (driver included) "
      f"that reach each:\n")
    w("| Native method | Programs |")
    w("|---|---:|")
    for method, names in sorted(missing.items(), key=lambda m: (-len(m[1]), m[0])):
        w(f"| `{method}` | {len(names)} |")
    w("")
    return "\n".join(out)


def main():
    args = sys.argv[1:]
    per_target_file = None
    if "--per-target" in args:
        at = args.index("--per-target")
        per_target_file = args[at + 1]
        del args[at:at + 2]
    no_run = "--no-run" in args
    args = [a for a in args if a != "--no-run"]
    if len(args) != 1:
        sys.exit(__doc__)
    jdk = os.path.abspath(args[0])
    if not no_run:
        run(jdk)
    censuses = load()
    with open(os.path.join(ROOT, "docs", "jdk-coverage.md"), "w") as f:
        f.write(report(censuses, jdk))
    if per_target_file:
        names = [n for n, _ in programs() if n in censuses]
        with open(per_target_file, "w") as f:
            f.write(per_target(censuses, names))


if __name__ == "__main__":
    main()
