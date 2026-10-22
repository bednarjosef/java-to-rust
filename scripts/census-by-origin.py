#!/usr/bin/env python3
"""Splits a census (java-to-rust --census-json) by where each piece of code comes from: the
first source root, of those given, whose directory has the piece's file (several roots may share
a name, such as the JDK's modules). A census tells program
code from library code by the files given on the command line; a program that is given as a
library root apart from its main class, as the Minecraft server is (scripts/minecraft-server-survey.sh),
is split here instead.

    census-by-origin.py CENSUS_JSON NAME=ROOT...   [--top N]

Prints, for each origin, the pieces, how many translate, and the rejections that block most of
its pieces (a piece's first error), and then the rejections over all of them."""

import collections
import json
import os
import sys


def main():
    args = sys.argv[1:]
    top = 15
    if "--top" in args:
        at = args.index("--top")
        top = int(args[at + 1])
        del args[at : at + 2]
    census = json.load(open(args[0]))
    roots = [tuple(a.split("=", 1)) for a in args[1:]]

    def origin(piece):
        file = piece["location"].rsplit(":", 1)[0]
        for name, root in roots:
            if os.path.exists(os.path.join(root, file)):
                return name
        return "other"

    pieces = collections.defaultdict(list)
    for piece in census["pieces"]:
        pieces[origin(piece)].append(piece)
    order = list(dict.fromkeys(name for name, _ in roots if name in pieces))
    order += ["other"] if "other" in pieces else []
    everything = [p for name in order for p in pieces[name]]
    for name in order + ["all"]:
        group = pieces[name] if name != "all" else everything
        rejected = [p for p in group if p["errors"]]
        share = 100.0 * (len(group) - len(rejected)) / len(group)
        print(f"{name}: {len(group)} pieces; {len(group) - len(rejected)} ({share:.1f}%) translate, "
              f"{len(rejected)} rejected")
    print()
    for name in order + ["all"]:
        group = pieces[name] if name != "all" else everything
        blocked = collections.Counter(p["errors"][0]["message"] for p in group if p["errors"])
        if not blocked:
            continue
        print(f"{name}: rejections by pieces of code they block (the first error of each)")
        for message, count in blocked.most_common(top):
            print(f"  {count:6}  {message}")
        print()


main()
