# java-to-rust

## Documentation

- Docs describe the current state: what the translator does now, how, and why. When a change
  alters something a doc describes, edit that section in place. Never append a dated note, a
  change log, or a "this now also handles…" paragraph; the history of a change belongs in its
  commit message.
- When a change makes a sentence in a doc false, fix or delete that sentence in the same commit.
- `README.md` stays short: what the project is, how to build, run and test it, and links into
  `docs/`. Feature lists and design detail belong in `docs/`.
- Plans for work not done yet stay in the doc they belong to, marked as plans. Once the work is
  done, the plan is replaced by a description of what was built, not kept alongside it.
- Briefs in `tools/worker/briefs/` are deleted once their branch has merged.
