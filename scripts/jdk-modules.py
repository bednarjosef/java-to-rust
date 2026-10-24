#!/usr/bin/env python3
"""The JDK modules that a program's sources use: those with a package that the sources name,
in an import or a qualified name in code or a string (`Class.forName`), and, transitively, the
modules that those require and those that provide a service that they use, as the JVM binds
services when it resolves its boot layer (`ManagementFactory` of java.management finds the
platform beans of jdk.management with `ServiceLoader`). java.base's services are the exception:
their providers are in nearly every module of the JDK (its tools, the locale data, charsets,
security providers, file systems), and a module that provides one is included only when the
sources name it or a module requires it. The packages of each module and what it declares are
read from the JDK's src.zip (one directory per module, as `java.base/java/lang/Object.java`).

    jdk-modules.py SRC_ZIP ROOT...

Prints the modules, one per line, java.base first and the others sorted. A module that only
comments name is left out."""

import os
import re
import sys
import zipfile

# A string or character literal, kept, or a comment, dropped.
LITERAL_OR_COMMENT = re.compile(r'"(?:\\.|[^"\\\n])*"|\'(?:\\.|[^\'\\\n])*\'|//[^\n]*|/\*.*?\*/', re.S)
# A package's name before a class's (`java.util.logging.Logger`) or an import's `*`. A name in
# the middle of a longer one (`a.java.util.X`) is not one.
PACKAGE = re.compile(r"(?<![\w$.])([a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+)\.(?:[A-Z_$]|\*)")
REQUIRES = re.compile(r"\brequires\s+(?:(?:transitive|static)\s+)*([\w.]+)\s*;")
USES = re.compile(r"\buses\s+([\w.$]+)\s*;")
PROVIDES = re.compile(r"\bprovides\s+([\w.$]+)\s+with\b")


def without_comments(source):
    return LITERAL_OR_COMMENT.sub(lambda m: m.group(0) if m.group(0)[0] in "\"'" else " ", source)


def main():
    zip_path, roots = sys.argv[1], sys.argv[2:]
    module_of = {}
    with zipfile.ZipFile(zip_path) as src:
        for name in src.namelist():
            module, _, path = name.partition("/")
            if path.endswith(".java") and "/" in path:
                module_of[path.rsplit("/", 1)[0].replace("/", ".")] = module
        modules = set(module_of.values())
        requires, uses, providers = {}, {}, {}
        for module in modules:
            try:
                info = without_comments(src.read(f"{module}/module-info.java").decode("utf-8"))
            except KeyError:
                continue
            requires[module] = REQUIRES.findall(info)
            uses[module] = USES.findall(info)
            for service in PROVIDES.findall(info):
                providers.setdefault(service, []).append(module)

    used = {"java.base"}
    for root in roots:
        for dir, _, files in os.walk(root):
            for file in files:
                if not file.endswith(".java"):
                    continue
                with open(os.path.join(dir, file), encoding="utf-8", errors="replace") as f:
                    source = without_comments(f.read())
                for package in PACKAGE.findall(source):
                    module = module_of.get(package)
                    if module:
                        used.add(module)

    pending = list(used)
    while pending:
        module = pending.pop()
        bound = [] if module == "java.base" else uses.get(module, [])
        for needed in requires.get(module, []) + [p for s in bound for p in providers.get(s, [])]:
            if needed in modules and needed not in used:
                used.add(needed)
                pending.append(needed)
    print("java.base")
    for module in sorted(used - {"java.base"}):
        print(module)


if __name__ == "__main__":
    main()
