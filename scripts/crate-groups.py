#!/usr/bin/env python3
"""The groups of a translation's code that depend on each other in a loop, which a split into
crates must keep together (see docs/several-crates.md).

Usage: scripts/crate-groups.py MODULES_JSON [--list N]

MODULES_JSON is what `java-to-rust ... --modules-json FILE` writes. The groups are the strongly
connected components of the graph whose nodes are the top-level classes (a nested class stays
with its top-level class; a package's statics are a node of their own) and whose edges are the
classes each item names; and of the same graph over packages. Each is reported as the generator
wrote it, and without the edges of the enums of class hierarchies, which is what dispatch through
tables of functions instead of enums would leave. --list N lists the N largest groups.
"""

import json
import sys
from collections import defaultdict


def load(path):
    modules = json.load(open(path))["modules"]
    package_of = {}
    for module in modules:
        for item in module["items"]:
            if item["class"]:
                package_of[item["class"]] = module["package"]
    return modules, package_of


def top_level(name, package_of):
    """The top-level class of a class, by its qualified name."""
    package = package_of.get(name)
    if not package:
        # A class of the crate root: of the unnamed package, or one the translator provides
        # (`Object`, the standard exceptions), named from its first capitalized segment.
        segments = name.split(".")
        first = next((i for i, s in enumerate(segments) if s[:1].isupper()), 0)
        return "(root)." + segments[first]
    return package + "." + name[len(package) + 1 :].split(".")[0]


def components(edges):
    """The strongly connected components of a graph (Tarjan's algorithm, without recursion)."""
    index, low, on_stack, stack, found = {}, {}, set(), [], []
    counter = 0
    for start in list(edges):
        if start in index:
            continue
        index[start] = low[start] = counter
        counter += 1
        stack.append(start)
        on_stack.add(start)
        work = [(start, iter(edges[start]))]
        while work:
            node, successors = work[-1]
            for successor in successors:
                if successor not in index:
                    index[successor] = low[successor] = counter
                    counter += 1
                    stack.append(successor)
                    on_stack.add(successor)
                    work.append((successor, iter(edges[successor])))
                    break
                if successor in on_stack:
                    low[node] = min(low[node], index[successor])
            else:
                work.pop()
                if work:
                    parent = work[-1][0]
                    low[parent] = min(low[parent], low[node])
                if low[node] == index[node]:
                    group = []
                    while True:
                        member = stack.pop()
                        on_stack.discard(member)
                        group.append(member)
                        if member == node:
                            break
                    found.append(group)
    return found


def graphs(modules, package_of, with_enums):
    """The class graph and the package graph, with the lines of each node."""
    classes, class_lines = defaultdict(set), defaultdict(int)
    packages, package_lines = defaultdict(set), defaultdict(int)
    for module in modules:
        package = module["package"] or "(root)"
        package_lines[package] += module["lines"]
        packages[package]
        for item in module["items"]:
            if item["class"]:
                node = top_level(item["class"], package_of)
            else:
                node = package + "#" + item["kind"]
            class_lines[node] += item["lines"]
            classes[node]
            if not with_enums and item["kind"] == "class enum":
                continue
            for name in item["classes"]:
                other = top_level(name, package_of)
                if other != node:
                    classes[node].add(other)
                    classes[other]
            for other in item["packages"]:
                other = other or "(root)"
                if other != package:
                    packages[package].add(other)
    return (classes, class_lines), (packages, package_lines)


def report(title, graph, kind, listed):
    edges, lines = graph
    total = sum(lines.values())
    groups = sorted(components(edges), key=lambda g: -sum(lines[n] for n in g))
    largest = sum(lines[n] for n in groups[0])
    second = sum(lines[n] for n in groups[1]) if len(groups) > 1 else 0
    alone = sum(1 for g in groups if len(g) == 1)
    print(
        f"{title}, {kind}: {len(edges)} {kind}, {total} lines, {len(groups)} groups;"
        f" the largest has {len(groups[0])} {kind} and {largest} lines ({100 * largest / max(total, 1):.0f}%),"
        f" the next {second} lines; {alone} groups of one"
    )
    for group in groups[:listed]:
        size = sum(lines[n] for n in group)
        names = ", ".join(sorted(group, key=lambda n: -lines[n])[:12])
        print(f"    {len(group)} {kind}, {size} lines: {names}{', ...' if len(group) > 12 else ''}")


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    listed = int(sys.argv[sys.argv.index("--list") + 1]) if "--list" in sys.argv else 0
    modules, package_of = load(sys.argv[1])
    for with_enums, title in [(True, "as translated"), (False, "without the hierarchies' enums")]:
        class_graph, package_graph = graphs(modules, package_of, with_enums)
        report(title, class_graph, "classes", listed)
        report(title, package_graph, "packages", listed)


if __name__ == "__main__":
    main()
