#!/usr/bin/env python3
"""Draws the README's performance charts into docs/images/, a light and a dark SVG of each.

Usage: python3 scripts/performance-charts.py

The numbers are copied from the tables they come from: docs/performance.md ("Results",
"Memory", "The translated javac") and docs/minecraft-server.md ("The server's speed"). When one
of those tables changes, change the numbers here and run the script again.

Every bar is a ratio against Java on a log scale that starts at 1x: to the right (blue) the
translation is that many times better, to the left (orange) that many times worse.
"""

from pathlib import Path

import matplotlib

matplotlib.use("svg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.ticker import FixedLocator, NullLocator  # noqa: E402

OUT = Path(__file__).resolve().parent.parent / "docs" / "images"

# docs/performance.md, "Results": wall clock in ms, Java then Rust, JVM start-up included.
BENCHMARKS = {
    "BaseFields": (170, 69), "BinaryTrees": (719, 1056), "ClassCast": (85, 83),
    "Collatz": (1155, 581), "EnumKeyLookups": (313, 226), "Interpreter": (1314, 1000),
    "LargeArrays": (34, 7), "Mandelbrot": (3400, 1143), "ManyThreadsAllocating": (155, 106),
    "MatrixMultiply": (124, 80), "MonitoredObjects": (67, 33), "NBody": (1811, 1884),
    "NodeList": (175, 110), "NoiseTree": (364, 425), "ObjectDispatch": (249, 135),
    "ObjectHash": (106, 48), "OwnerThreadChecks": (77, 73), "ParticleArray": (1426, 892),
    "ReflectionLookups": (166, 116), "ReflectionTableMemory": (25, 2), "Sieve": (1278, 1006),
    "SmallObjects": (145, 131), "SortMillion": (1691, 1439), "Statics": (1286, 1266),
    "StringBuild": (28, 1), "StringKeyHash": (38, 14), "TokenScanner": (994, 775),
    "TypeTests": (168, 46), "UnlockedObjects": (69, 34),
}

# docs/performance.md, "Memory": peak resident MB, Java then the translation ("after").
MEMORY = {
    "ReflectionTableMemory": (42.0, 14.9), "LargeArrays": (143, 88.4),
    "MonitoredObjects": (148, 76.6), "UnlockedObjects": (166, 76.5),
    "SmallObjects": (734, 145.6), "ManyThreadsAllocating": (417, 197.7),
    "DeepThreadStacks": (514, 199.7),
}

# docs/performance.md, "The translated javac": seconds, the real javac then the collector's
# quick build. docs/minecraft-server.md, "The server's speed": Java 25 then trial-pgo, profiled,
# the middle of each range.
LARGE = [
    ("javac", "one class", 0.19 / 0.023),
    ("javac", "47 programs", 0.63 / 0.34),
    ("javac", "540 files", 1.85 / 2.40),
    ("javac", "4,700 files", 11.3 / 59.7),
    ("Minecraft server", "start-up time", 1.65 / 0.585),
    ("Minecraft server", "peak memory", 2.7 / 0.825),
    ("Minecraft server", "ticks per second", 36_050 / 42_650),
]

THEMES = {
    "light": dict(text="#1f2328", muted="#59636e", grid="#d1d9e0", better="#2a78d6",
                  worse="#eb6834"),
    "dark": dict(text="#f0f6fc", muted="#9198a1", grid="#3d444d", better="#3987e5",
                 worse="#d95926"),
}


def label(r):
    if r >= 10:
        return f"{r:.0f}x"
    return f"{r:.1f}x" if r >= 2 else f"{r:.2f}x"


def draw(rows, title, note, name, groups=None):
    """rows: (label, ratio) from top to bottom; groups: {row index: heading above it}."""
    groups = groups or {}
    for mode, c in THEMES.items():
        n = len(rows) + len(groups)
        fig, ax = plt.subplots(figsize=(8, 0.9 + 0.3 * n), dpi=100)
        fig.patch.set_alpha(0)
        ax.patch.set_alpha(0)
        lo = min(min(r for _, r in rows) / 1.6, 0.5)
        hi = max(max(r for _, r in rows) * 2.2, 4)
        y, ticks, names = 0, [], []
        for i, (text, r) in enumerate(rows):
            if i in groups:
                ax.text(lo, y, groups[i], color=c["text"], fontsize=10, fontweight="bold",
                        va="center", ha="left")
                y += 1
            color = c["better"] if r >= 1 else c["worse"]
            ax.barh(y, r - 1, left=1, height=0.62, color=color, linewidth=0)
            ax.text(r * 1.06 if r >= 1 else r / 1.06, y, label(r), color=c["text"],
                    fontsize=9, va="center", ha="left" if r >= 1 else "right")
            ticks.append(y)
            names.append(text)
            y += 1
        ax.set_xscale("log")
        ax.set_xlim(lo, hi)
        ax.set_ylim(y - 0.4, -0.6)
        ax.set_yticks(ticks, names, fontsize=9, color=c["text"])
        steps = [s for s in (0.2, 0.5, 1, 2, 5, 10, 20, 50) if lo <= s <= hi]
        ax.xaxis.set_major_locator(FixedLocator(steps))
        ax.xaxis.set_minor_locator(NullLocator())
        ax.set_xticklabels([f"{s:g}x" for s in steps], fontsize=8, color=c["muted"])
        ax.axvline(1, color=c["muted"], linewidth=1)
        ax.grid(axis="x", color=c["grid"], linewidth=0.6)
        ax.set_axisbelow(True)
        ax.tick_params(length=0)
        for side in ax.spines.values():
            side.set_visible(False)
        ax.set_title(title, loc="left", fontsize=12, color=c["text"], pad=10)
        fig.text(0.01, 0.01, note, fontsize=8, color=c["muted"], ha="left", va="bottom")
        fig.tight_layout(rect=(0, 0.04, 1, 1))
        fig.savefig(OUT / f"{name}-{mode}.svg", metadata={"Date": None})
        plt.close(fig)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    plt.rcParams["svg.hashsalt"] = "java-to-rust"
    plt.rcParams["font.family"] = "DejaVu Sans"
    speed = sorted(((k, j / r) for k, (j, r) in BENCHMARKS.items()), key=lambda t: -t[1])
    draw(speed, "Benchmarks: times faster than Java",
         "Wall clock, median of 5 runs, JVM start-up included (docs/performance.md)",
         "benchmarks")
    memory = sorted(((k, j / r) for k, (j, r) in MEMORY.items()), key=lambda t: -t[1])
    draw(memory, "Benchmarks: times less memory than Java",
         "Peak resident memory (docs/performance.md, \"Memory\")", "memory")
    draw([(text, r) for _, text, r in LARGE], "Large programs: times better than Java",
         "javac: compile time. Minecraft: start-up, memory, server ticks per second "
         "(docs/performance.md, docs/minecraft-server.md)",
         "large-programs", groups={0: "javac", 4: "Minecraft server"})


if __name__ == "__main__":
    main()
