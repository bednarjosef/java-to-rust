#!/usr/bin/env python3
"""Claude Code hooks for the workers that tools/worker/worker.py starts. They run in the
worker's worktree; its state is in target/worker/ (brief.md, checklist.md, notes.md, base).

pre:     PreToolUse on Bash. Refuses what wastes context or escapes the worktree, naming the
         command to use instead.
compact: SessionStart after a compaction. Gives back the brief, the checklist, the notes and the
         worker's commits, which a summary loses.
stop:    Stop. While something is missing (open checklist items, uncommitted changes, no commit,
         `scripts/check.sh` not passed on HEAD), the worker may not end its turn; at most
         STOP_BLOCKS times per session, so a truly blocked worker can still finish.
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

STATE = Path("target/worker")
STOP_BLOCKS = 3


def git(*args):
    r = subprocess.run(["git", *args], capture_output=True, text=True)
    return r.stdout.strip()


def read(name):
    try:
        return (STATE / name).read_text()
    except OSError:
        return ""


def deny(reason):
    # Said every time: after a few refusals, models conclude that the whole shell is banned.
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "deny",
        "permissionDecisionReason": "Refused (plain commands such as `git status`, `cargo test` "
                                    "and `scripts/check.sh` still work): " + reason}}))
    sys.exit(0)


def lines(path):
    try:
        with open(path, "rb") as f:
            return sum(1 for _ in f)
    except OSError:
        return 0


def pre(event):
    cmd = (event.get("tool_input") or {}).get("command", "")
    if re.match(r"\s*cd\s", cmd):
        deny("your shell already starts in your worktree root; run the command there without cd.")
    if re.search(r"\bgit\s+-C\b", cmd):
        deny("run git in your worktree root without -C.")
    if re.search(r"\bgit\s+(push|checkout|switch|reset|rebase|merge|cherry-pick|worktree|branch\s+-[dDmM])\b", cmd):
        deny("you work on your own branch only: commit with `git add` and `git commit`; never push, "
             "switch branches, reset, rebase or merge.")
    m = re.fullmatch(r"\s*cat\s+([^\s|;&<>]+)\s*", cmd)
    if m and lines(m.group(1)) > 200:
        deny(f"{m.group(1)} has {lines(m.group(1))} lines. Find what you need with "
             "`grep -n PATTERN FILE`, then Read only that range (offset/limit).")


def missing():
    todo = []
    open_items = re.findall(r"^- \[ \] (.*)$", read("checklist.md"), re.M)
    todo += [f"checklist item not done: {item}" for item in open_items]
    if git("status", "--porcelain"):
        todo.append("uncommitted changes (`git status`): commit them")
    base = read("base").strip()
    if base and not git("rev-list", f"{base}..HEAD"):
        todo.append("no commits on your branch yet")
    passed = Path("target/check-passed")
    head = git("rev-parse", "HEAD")
    if not passed.exists() or passed.read_text().strip() != head:
        todo.append("`scripts/check.sh` has not passed on your last commit: run it (after committing)")
    return todo


def stop(event):
    marker = STATE / f"stops-{event.get('session_id', 'x')}"
    count = int(marker.read_text()) if marker.exists() else 0
    if count >= STOP_BLOCKS:
        return
    todo = missing()
    if not todo:
        return
    marker.write_text(str(count + 1))
    sys.stderr.write(
        "You are not done:\n" + "\n".join("- " + t for t in todo)
        + "\nFinish these now, then send your report. If one is truly blocked, mark the checklist "
        "item `- [blocked: why]`, say why in your report and end your turn again "
        f"({STOP_BLOCKS - count - 1} more stops will be held).\n")
    sys.exit(2)


def compact(event):
    base = read("base").strip()
    commits = git("log", "--oneline", f"{base}..HEAD") if base else ""
    text = (
        "Your context was just compacted. What you must not lose, read back from disk "
        "(target/worker/):\n\n## Brief\n\n" + read("brief.md")
        + "\n\n## Checklist (target/worker/checklist.md)\n\n" + read("checklist.md")
        + "\n\n## Your notes (target/worker/notes.md)\n\n" + read("notes.md")
        + "\n\n## Your commits\n\n" + (commits or "(none yet)")
        + "\n\n## Uncommitted changes\n\n" + (git("status", "--short") or "(none)")
        + "\n\nCarry on from the first open checklist item. Re-read only the lines you need "
        "(grep -n, then Read with offset/limit).")
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "SessionStart",
                                             "additionalContext": text}}))


def main():
    os.chdir(os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd())
    event = json.load(sys.stdin)
    if not STATE.is_dir():
        return  # not a worker's worktree
    {"pre": pre, "compact": compact, "stop": stop}[sys.argv[1]](event)


if __name__ == "__main__":
    main()
