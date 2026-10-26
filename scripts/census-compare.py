#!/usr/bin/env python3
"""Compares two censuses (java-to-rust --census-json), such as the one before a change and the
one after it. Each of OLD and NEW is a census.json, or a directory that has one.

    census-compare.py OLD NEW [NAME=ROOT...]   [--top N]

Prints whether each census is complete, the pieces of each origin and the share of them that
translate, side by side, then the rejection messages whose pieces went down the most and up the
most, and the messages that are new. A message's pieces are those it blocks, as the census
counts them (every piece once for each normalized message it has). Last come the handle calls
that the translation does not follow and makes throw UnsupportedOperationException, which are no
rejections: the change in their pieces, and the messages whose pieces changed. The origins are the census's
own (program and library), or, with NAME=ROOT arguments, those of census-by-origin.py: the first
root whose directory has the piece's file."""

import collections
import importlib.util
import json
import os
import sys


def by_origin():
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "census-by-origin.py")
    spec = importlib.util.spec_from_file_location("census_by_origin", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load(path):
    if os.path.isdir(path):
        path = os.path.join(path, "census.json")
    if not os.path.isfile(path):
        sys.exit(f"no census at {path}")
    with open(path) as file:
        return json.load(file)


def counts(census, origin, order):
    """The pieces of each origin, and how many of them translate, with "all" last."""
    rows = collections.defaultdict(lambda: [0, 0])
    for piece in census["pieces"]:
        for name in (origin(piece), "all"):
            rows[name][0] += 1
            rows[name][1] += not piece["errors"]
            if name not in order:
                order.append(name)
    return rows


def messages(census):
    pieces = collections.Counter()
    for row in census["rejections"]:
        pieces[row["message"]] += row["pieces"]
    return pieces


def unfollowed(census):
    """The pieces of each message of a handle call not followed (none in an older census)."""
    pieces = collections.Counter()
    for row in census.get("unfollowedHandles", []):
        pieces[row["message"]] += row["pieces"]
    return pieces


def state(census):
    if census["complete"]:
        return "complete"
    fatal = census["fatal"]
    return f"stopped at {fatal['place']}: {fatal['message']}" if fatal else "incomplete"


def percent(translate, pieces):
    return 100.0 * translate / pieces if pieces else 0.0


def main():
    args = sys.argv[1:]
    top = 20
    if "--top" in args:
        at = args.index("--top")
        top = int(args[at + 1])
        del args[at : at + 2]
    if len(args) < 2:
        sys.exit(__doc__)
    old, new = load(args[0]), load(args[1])
    roots = [tuple(a.split("=", 1)) for a in args[2:]]
    origin = by_origin().origins(roots) if roots else lambda piece: piece["source"]

    print(f"Census comparison: {args[0]} (old) against {args[1]} (new)")
    print(f"  old: {state(old)}")
    print(f"  new: {state(new)}")
    if not (old["complete"] and new["complete"]):
        print("  A census that stopped has only the rejections found before it stopped: it shows more")
        print("  pieces translating than a complete one would.")
    print()

    # The roots' order first, as census-by-origin.py prints them, then any other origin as met.
    order = list(dict.fromkeys(name for name, _ in roots))
    before, after = counts(old, origin, order), counts(new, origin, order)
    order = [name for name in order if name != "all" and (name in before or name in after)] + ["all"]
    width = max(len(name) for name in order)
    print("Pieces, the share of them that translate, and the change in points and in pieces that translate:")
    print(f"  {'':{width}}  {'old pieces':>10} {'translate':>9}  {'new pieces':>10} {'translate':>9}"
          f"  {'points':>7}  {'translate':>9}")
    for name in order:
        (old_pieces, old_translate), (new_pieces, new_translate) = before[name], after[name]
        old_share, new_share = percent(old_translate, old_pieces), percent(new_translate, new_pieces)
        print(f"  {name:{width}}  {old_pieces:10} {old_share:8.1f}%  {new_pieces:10} {new_share:8.1f}%"
              f"  {new_share - old_share:+7.1f}  {new_translate - old_translate:+9}")
    print()

    before, after = messages(old), messages(new)
    change = {m: after[m] - before[m] for m in before.keys() | after.keys()}
    down = sorted((m for m in before if change[m] < 0), key=lambda m: (change[m], m))
    up = sorted((m for m in before if change[m] > 0), key=lambda m: (-change[m], m))
    fresh = sorted((m for m in after if m not in before), key=lambda m: (-after[m], m))
    for title, rows in (
        ("Messages whose pieces went down the most", down),
        ("Messages whose pieces went up the most", up),
        ("New messages", fresh),
    ):
        print(f"{title} ({len(rows)}):" if rows else f"{title}: none")
        for m in rows[:top]:
            gone = "  (gone)" if not after[m] else ""
            print(f"  {before[m]:7} -> {after[m]:7}  {change[m]:+7}  {m}{gone}")
        if len(rows) > top:
            print(f"  and {len(rows) - top} more")
        print()

    before, after = unfollowed(old), unfollowed(new)
    print("Handle calls not followed, which throw UnsupportedOperationException where Java runs them, "
          f"by pieces: {sum(before.values())} -> {sum(after.values())}")
    changed = sorted((m for m in before.keys() | after.keys() if before[m] != after[m]),
                     key=lambda m: (-abs(after[m] - before[m]), m))
    for m in changed[:top]:
        note = "  (new)" if not before[m] else "  (gone)" if not after[m] else ""
        print(f"  {before[m]:7} -> {after[m]:7}  {after[m] - before[m]:+7}  {m}{note}")
    if len(changed) > top:
        print(f"  and {len(changed) - top} more")


if __name__ == "__main__":
    main()
