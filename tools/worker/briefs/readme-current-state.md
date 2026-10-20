# Bring README "Current state" up to date

README.md's "Current state" section has fallen behind main. Many branches landed tonight: strings
and bytes, string and static performance, array ownership (plain `Vec` arrays, owned returns,
moves, borrowed parameters, sliced loops), idiomatic loops and statements, idiomatic expressions,
expression fixes, corpus test speed, the fuzzers. Several statements in it are now wrong or
missing. Make the README describe main as it is, accurately and briefly.

## Scope
- Read README.md whole, then check each claim against main: `git log --oneline -60`,
  `docs/performance.md`, `docs/lists-and-streams.md`, `docs/thorough-mode.md`,
  `docs/multi-threading.md`, `known-failures/README.md`, `targets/README.md`, and the code where
  a claim is unclear. Translate a small program yourself when you need to see current output
  (`target/debug/java-to-rust File.java`).
- Fix what is wrong, add what is missing (one or two sentences per area, in the README's
  existing style: plain, exact, no marketing), remove duplication. Keep the Level 1 / Level 2
  description but say what the default object representation actually is today
  (`Rc<RefCell<C>>`, `Rc<C>` when immutable, plain values when owned: check
  `crates/translator/src/analysis/objects.rs` and `owned.rs`), and add a short "Plans" list that
  links the documents in docs/ with one line each.
- Do not describe the object model, exceptions or generics as done: they are on branches that
  have not landed (the statement that inheritance, interfaces and generics are rejected stays
  true on main until then).
- Only edit README.md. Put temporary files in
  <scratchpad>/readme-current-state/.
- Builds and tests: run them through the machine-wide slot limiter, e.g.
  `<scratchpad>/build-slot.sh scripts/check.sh`
  (it waits for a free slot; a wait is normal).

## Deliverables
- Every claim in Current state checked against main
- Wrong or stale statements fixed, missing areas added
- Plans list linking the docs
