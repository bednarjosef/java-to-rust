# A faster corpus test

`crates/translator/tests/corpus.rs` runs every program in `corpus/` on Java and as translated
Rust and compares stdout and exit code. With about 170 programs it takes about 5 minutes, and
every agent runs it before every commit. Make it much faster without weakening it.

## Scope
- Measure first where the time goes (javac/java start-up, translation, cargo builds, linking,
  running) and write the numbers in your notes.
- Likely wins, keep whichever measure best: build all translated programs as binaries of one
  Cargo workspace or one crate with many `[[bin]]` targets so the runtime crate is compiled
  once and linking runs in parallel; run Java programs in parallel (bounded by the CPU count);
  compile all Java files with one `javac` call; use a persistent target directory under
  `target/` so unchanged programs are not rebuilt (key the cache on the translated source, so a
  changed translation always rebuilds); keep overflow checks on as today.
- A name filter: `CORPUS_FILTER=substring cargo test --test corpus` runs only matching programs
  (all when unset), and the failure output names every failing program with a diff, not just
  the first.
- Keep the semantics: every program in corpus/programs, corpus/features and corpus/projects is
  still checked, with the same comparison; `scripts/check.sh` still runs it all.
- Only change `crates/translator/tests/corpus.rs` (and helpers next to it), `scripts/check.sh`
  if needed, and README's paragraph on the corpus test. Other agents add corpus programs at the
  same time; that must keep working unchanged.

## Deliverables
- Measured breakdown of the current corpus test time
- Corpus test at least three times faster on this machine, same checks
- CORPUS_FILTER name filter and a report listing every failing program
- README corpus paragraph updated
