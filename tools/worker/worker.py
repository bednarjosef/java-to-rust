#!/usr/bin/env python3
"""Runs Claude Haiku workers on this repository, each in its own git worktree and branch.

Workers are Claude Code background sessions (`claude --bg`): `claude agents` lists them and
`claude attach NAME` opens one to watch or talk to it. Each gets a brief, a checklist seeded from
the brief's deliverables and a notes file (in its worktree's target/worker/), hooks that give
those back after every compaction and that keep it from finishing before its work is committed
and `scripts/check.sh` passes (hook.py), and a compaction threshold under 100K tokens.

    tools/worker/worker.py start NAME BRIEF.md [--effort high] [--model MODEL] [--base REF]
    tools/worker/worker.py status            # every worker: state, commits, checklist, tokens
    tools/worker/worker.py usage [NAME]      # token usage per worker, in detail
    tools/worker/worker.py report NAME       # the worker's last message
    tools/worker/worker.py stop NAME
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
WORKTREES = REPO / ".worktrees"
STATE = WORKTREES / ".workers"
MODEL = "claude-haiku-5-5"
# Claude Code compacts at about this window minus a reserve for the summary: 122000 gives
# compaction near 89K tokens, below the 100K mark where long prompts get expensive and slow.
COMPACT_WINDOW = "122000"
TOOLS = ["Read", "Edit", "Write", "Bash", "Grep", "Glob", "SendMessage"]
COMMANDS = [
    "cargo *", "scripts/check.sh", "./scripts/check.sh", "java *", "javac *", "timeout * java *",
    "timeout * cargo *", "timeout * target/*", "./target/*", "target/*",
    "git status*", "git diff*", "git log*", "git show*", "git add *", "git commit *",
    "git restore *", "git rm *", "git mv *", "git grep *", "git stash*", "git blame *",
    "git rev-parse *", "git branch --show-current",
    "ls*", "cat *", "head *", "tail *", "grep *", "rg *", "find *", "wc *", "diff *", "sort *",
    "uniq *", "cut *", "tr *", "sed -n *", "sed -i *", "jq *", "echo*", "printf *", "true",
    "pwd", "which *", "mkdir -p *", "touch *", "rm target/*", "rm -f target/*", "rm -r target/*",
    "rm -rf target/*", "rm crates/*", "rm -f crates/*", "rm corpus/*", "rm -f corpus/*",
    "mv crates/*", "mv corpus/*",
]
IMAGE_DENY = [f"Read(**/*.{e})" for e in ("png", "jpg", "jpeg", "gif", "webp")]


def die(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def run(*args, cwd=REPO, check=True):
    r = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if check and r.returncode != 0:
        die(f"{' '.join(args)} failed:\n{r.stdout}{r.stderr}")
    return r.stdout.strip()


def spec_path(name):
    return STATE / f"{name}.json"


def load_spec(name):
    try:
        return json.loads(spec_path(name).read_text())
    except (OSError, ValueError):
        die(f"no worker named {name}; `worker.py status` lists them")


def specs():
    out = []
    for p in sorted(STATE.glob("*.json")):
        if p.name.endswith(".settings.json"):
            continue
        try:
            out.append(json.loads(p.read_text()))
        except ValueError:
            pass
    return out


def caller_session():
    """Name of the Claude Code session running this command, which the worker reports to."""
    pid = os.environ.get("CLAUDE_PID")
    if not pid:
        return None
    try:
        return json.loads((Path.home() / ".claude" / "sessions" / f"{pid}.json").read_text()).get("name")
    except (OSError, ValueError):
        return None


def deliverables(brief):
    """The bullet lines under the brief's `## Deliverables` heading."""
    m = re.search(r"^## Deliverables\s*\n(.*?)(?=^## |\Z)", brief, re.M | re.S)
    if not m:
        return []
    return [line[2:].strip() for line in m.group(1).splitlines() if line.startswith("- ")]


def settings(name):
    hook = f"python3 {HERE / 'hook.py'}"
    env = {
        "CLAUDE_CODE_AUTO_COMPACT_WINDOW": COMPACT_WINDOW,
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY": "1",
        "CLAUDE_CODE_FILE_READ_MAX_OUTPUT_TOKENS": "6000",
        "BASH_MAX_OUTPUT_LENGTH": "12000",
        "CARGO_BUILD_JOBS": "4",
        "WORKER_NAME": name,
    }
    return {
        "env": env,
        "attribution": {"commit": "", "pr": "", "sessionUrl": False},
        "crossSessionInbound": "accept",
        "hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": [
                {"type": "command", "command": f"{hook} pre", "timeout": 20}]}],
            "SessionStart": [{"matcher": "compact", "hooks": [
                {"type": "command", "command": f"{hook} compact", "timeout": 20}]}],
            "Stop": [{"hooks": [{"type": "command", "command": f"{hook} stop", "timeout": 60}]}],
        },
    }


def prompt(name, brief, reply_to):
    lines = [
        f"You are {name}, a worker on the java-to-rust translator. Your task is the brief below; "
        "do all of it, alone. The rules in your system prompt apply throughout.",
        "Start by reading target/worker/checklist.md and target/worker/notes.md, then the code "
        "the brief points to.",
    ]
    if reply_to:
        lines.append(
            f'When `scripts/check.sh` passes on your final commit, send your report to "{reply_to}" '
            f'with SendMessage (to: "{reply_to}"), in one message, then end your turn. If you are '
            "blocked, send that instead of stopping silently. Later messages from that session "
            "are follow-up tasks.")
    else:
        lines.append("End with your report as your final message.")
    return "\n\n".join(lines) + "\n\n# Brief\n\n" + brief


def cmd_start(args):
    name = args.name
    if not re.fullmatch(r"[a-z][a-z0-9-]*", name):
        die("worker names are lowercase words joined by dashes, such as `arrays`")
    brief_path = Path(args.brief).resolve()
    brief = brief_path.read_text()
    if spec_path(name).exists():
        die(f"worker {name} already exists ({spec_path(name)}); stop it and remove the file first")
    STATE.mkdir(parents=True, exist_ok=True)
    wt = WORKTREES / name
    base = run("git", "rev-parse", args.base)
    if not wt.exists():
        run("git", "worktree", "add", "-q", "-b", name, str(wt), base)
    state = wt / "target" / "worker"
    state.mkdir(parents=True, exist_ok=True)
    (state / "brief.md").write_text(brief)
    (state / "base").write_text(base + "\n")
    if not (state / "checklist.md").exists():
        items = deliverables(brief) or ["Do the task in the brief"]
        (state / "checklist.md").write_text(
            "# Checklist\n\nOne line per deliverable of the brief. Tick `- [x]` when done; "
            "`- [blocked: why]` only when it truly cannot be done.\n\n"
            + "".join(f"- [ ] {item}\n" for item in items))
    if not (state / "notes.md").exists():
        (state / "notes.md").write_text(
            "# Notes\n\nOne line per fact worth keeping across compactions: where things live, "
            "decisions and why, dead ends.\n\n")
    settings_file = STATE / f"{name}.settings.json"
    settings_file.write_text(json.dumps(settings(name), indent=2))
    reply_to = args.reply_to or caller_session()
    allow = TOOLS[:-1] + [f"Bash({c})" for c in COMMANDS] + [
        f"Edit(/{wt}/**)", f"Write(/{wt}/**)", f"Read(/{wt}/**)"]
    flags = [
        "--bg", "--name", f"{name}-worker", "--model", args.model, "--effort", args.effort,
        "--settings", str(settings_file), "--setting-sources", "", "--strict-mcp-config",
        "--disable-slash-commands", "--permission-mode", "dontAsk",
        "--tools", *TOOLS, "--allowedTools", *allow, "--disallowedTools", *IMAGE_DENY,
        "--append-system-prompt-file", str(HERE / "RULES.md"),
    ]
    r = subprocess.run(["claude", *flags, "--", prompt(name, brief, reply_to)], cwd=wt,
                       capture_output=True, text=True, timeout=120)
    m = re.search(r"backgrounded · (\S+) · ", r.stdout)
    if r.returncode != 0 or not m:
        die(f"launch failed: {(r.stdout + r.stderr).strip()[:800]}")
    spec = {"name": name, "id": m.group(1), "worktree": str(wt), "branch": name, "base": base,
            "model": args.model, "effort": args.effort, "brief": str(brief_path),
            "reply_to": reply_to, "started": time.time()}
    spec_path(name).write_text(json.dumps(spec, indent=2))
    print(f"worker {name} started: session {name}-worker (id {spec['id']}), branch {name}, "
          f"worktree {wt.relative_to(REPO)}, {args.model} at effort {args.effort}"
          + (f", reports to {reply_to}" if reply_to else ""))
    print(f"watch it: claude attach {spec['id']}")


def sessions():
    r = subprocess.run(["claude", "agents", "--json", "--all"], capture_output=True, text=True)
    try:
        return {s["id"]: s for s in json.loads(r.stdout)}
    except (ValueError, KeyError):
        return {}


def transcript(session_id):
    hits = list((Path.home() / ".claude" / "projects").glob(f"*/{session_id}.jsonl"))
    return max(hits, key=lambda p: p.stat().st_mtime) if hits else None


def usage(session_id):
    """Token usage from the session transcript. Each API request is one assistant message (its
    content blocks are logged as separate lines with the same id, so they are counted once)."""
    path = transcript(session_id) if session_id else None
    requests = {}
    compactions = 0
    last_text = ""
    if path:
        for line in path.open():
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if e.get("type") == "system" and e.get("subtype") == "compact_boundary":
                compactions += 1
            msg = e.get("message") or {}
            if e.get("type") == "assistant" and msg.get("usage"):
                requests[msg.get("id") or len(requests)] = msg["usage"]
                for block in msg.get("content") or []:
                    if block.get("type") == "text" and block.get("text", "").strip():
                        last_text = block["text"]
    u = list(requests.values())
    prompt_sizes = [r.get("input_tokens", 0) + r.get("cache_read_input_tokens", 0)
                    + r.get("cache_creation_input_tokens", 0) for r in u]
    return {
        "requests": len(u),
        "compactions": compactions,
        "input": sum(r.get("input_tokens", 0) for r in u),
        "cache_read": sum(r.get("cache_read_input_tokens", 0) for r in u),
        "cache_write": sum(r.get("cache_creation_input_tokens", 0) for r in u),
        "output": sum(r.get("output_tokens", 0) for r in u),
        "prompt_last": prompt_sizes[-1] if prompt_sizes else 0,
        "prompt_peak": max(prompt_sizes, default=0),
        "prompt_mean": sum(prompt_sizes) // len(prompt_sizes) if prompt_sizes else 0,
        "over_100k": sum(p > 100_000 for p in prompt_sizes),
        "last_text": last_text,
    }


def k(n):
    return f"{n / 1e6:.2f}M" if n >= 1e6 else f"{n / 1e3:.1f}K"


def checklist(wt):
    try:
        text = (Path(wt) / "target" / "worker" / "checklist.md").read_text()
    except OSError:
        return "?"
    done = len(re.findall(r"^- \[x\]", text, re.M | re.I))
    blocked = len(re.findall(r"^- \[blocked", text, re.M))
    total = done + blocked + len(re.findall(r"^- \[ \]", text, re.M))
    return f"{done}/{total}" + (f" ({blocked} blocked)" if blocked else "")


def cmd_status(args):
    live = sessions()
    rows = specs()
    if not rows:
        print("no workers")
        return
    totals = {"input": 0, "cache_read": 0, "cache_write": 0, "output": 0}
    for s in rows:
        sess = live.get(s["id"], {})
        state = sess.get("state") or "gone"
        if sess.get("status"):
            state += f"/{sess['status']}"
        commits = run("git", "rev-list", "--count", f"{s['base']}..{s['branch']}", check=False) or "?"
        u = usage(sess.get("sessionId"))
        for key in totals:
            totals[key] += u[key]
        print(f"{s['name']:<10} {state:<16} commits {commits:<3} checklist {checklist(s['worktree']):<14} "
              f"requests {u['requests']:<4} compactions {u['compactions']:<2} prompt now {k(u['prompt_last'])} "
              f"peak {k(u['prompt_peak'])} (>100K: {u['over_100k']}) output {k(u['output'])}")
    print(f"all workers: input {k(totals['input'])}, cache read {k(totals['cache_read'])}, "
          f"cache write {k(totals['cache_write'])}, output {k(totals['output'])}")


def cmd_usage(args):
    live = sessions()
    for s in specs():
        if args.name and s["name"] != args.name:
            continue
        u = usage(live.get(s["id"], {}).get("sessionId"))
        print(f"{s['name']} ({s['model']}, effort {s['effort']}):")
        print(f"  requests {u['requests']}, compactions {u['compactions']}")
        print(f"  prompt per request: mean {k(u['prompt_mean'])}, peak {k(u['prompt_peak'])}, "
              f"now {k(u['prompt_last'])}, over 100K {u['over_100k']} times")
        print(f"  tokens: input {k(u['input'])}, cache read {k(u['cache_read'])}, "
              f"cache write {k(u['cache_write'])}, output {k(u['output'])} (thinking included)")


def cmd_report(args):
    s = load_spec(args.name)
    u = usage(sessions().get(s["id"], {}).get("sessionId"))
    print(u["last_text"] or "(no message yet)")


def cmd_stop(args):
    s = load_spec(args.name)
    print(run("claude", "stop", s["id"], check=False))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    a = sub.add_parser("start")
    a.add_argument("name")
    a.add_argument("brief")
    a.add_argument("--effort", default="high")
    a.add_argument("--model", default=MODEL)
    a.add_argument("--base", default="HEAD")
    a.add_argument("--reply-to")
    sub.add_parser("status")
    a = sub.add_parser("usage")
    a.add_argument("name", nargs="?")
    a = sub.add_parser("report")
    a.add_argument("name")
    a = sub.add_parser("stop")
    a.add_argument("name")
    args = p.parse_args()
    {"start": cmd_start, "status": cmd_status, "usage": cmd_usage, "report": cmd_report,
     "stop": cmd_stop}[args.cmd](args)


if __name__ == "__main__":
    main()
