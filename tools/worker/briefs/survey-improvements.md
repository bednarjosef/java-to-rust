# Improvements to the survey

`--survey` (`crates/translator/src/program/survey.rs`, wired in `program.rs` and `main.rs`)
prints which unsupported features the code reachable from `main` uses. Make it more useful to
the people implementing those features. Only touch `program/survey.rs`, the `--survey` wiring
in `main.rs`/`program.rs`, its test in `tests/translation.rs` and the README paragraph on it.

## Scope
- Bug: a local class declared inside a method body (`class Sink {..}` inside a method, common
  in java.util.stream) is not in `declared`, so calls to it count as "classes outside the
  sources". Collect local and anonymous classes as declared too.
- Split every table into the program's own code and library code: units of the files and
  source roots given are the program, units javac read from `--library` roots are library.
  Print both feature tables (program first), and keep the greedy order for the whole.
- For each feature, print up to three example locations (`file:line`, shortest path relative
  to the root it came from) of reachable code that uses it, so an implementer can open them.
- `--survey-json PATH`: also write the census as JSON (pieces with class, kind, name,
  location, features; runtime calls; natives) for scripts.
- A test in `tests/translation.rs` for each of these (small programs written with the existing
  `write_sources` helper; library roots can be a second directory).
- Try it on a program that uses the JDK: the JDK's java.base sources are at
  <scratchpad>/jdk-src/java.base
  and sample programs at .../scratchpad/survey/all (main class `All`). Never copy JDK sources
  into the repository.

## Deliverables
- Local and anonymous classes counted as declared
- Separate program and library tables
- Example locations per feature
- --survey-json output
- Tests for each, README paragraph updated
