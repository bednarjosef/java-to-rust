# Using the translator

There are two modes. A **single file** uses only the program's own classes and what the runtime
crate supplies. A **whole program** is translated together with the JDK's own sources
(`java.base`), so collections, streams and the rest of the library come from the JDK's code. What
each mode accepts is in [supported-java.md](supported-java.md).

Building the translator and running its tests need `javac` and `java` (JDK 21 or newer) on PATH.
Translation never runs the program; it only compiles it with javac, which takes a few seconds.

```
cargo run -p java-to-rust -- Program.java                 # print the Rust translation
cargo run -p java-to-rust -- Program.java --crate out/    # write a Cargo project
cargo run -p java-to-rust -- --main com.example.Main --source src/ [--library jdk-src/java.base] [-o out.rs | --crate out/]
cargo run -p java-to-rust -- --project my-app/ --crate out/  # a Maven or Gradle project
cargo run -p java-to-rust -- --census --main Main --source src/ --library jdk-src/java.base
cargo run -p java-to-rust -- --survey --main Main --source src/ --library jdk-src/java.base
```

- `--crate DIR` writes a Cargo project with a file per package; `-o FILE` writes one file. With
  `--crates N`, `--crate DIR` writes a workspace of a header crate, `N` body crates and the binary
  instead, for programs too large to build as one crate ([docs/several-crates.md](several-crates.md)).
- A written crate depends on the runtime crate by its path: `crates/runtime` next to the source
  the translator was built from, or `JAVA_TO_RUST_RUNTIME` when it is set. The translator checks
  that the runtime crate is there before it starts, rather than after a long translation. The
  translator's other files (its Java helpers and its overlay and patches of the JDK) are inside
  its binary and are written to `~/.cache/java-to-rust/files` on first use (`JAVA_TO_RUST_CACHE`
  names another directory), so the binary can be copied anywhere.
- `--jdk` adds the JDK's own sources as libraries: those of the JDK that `java` on `PATH` runs,
  unpacked from its `lib/src.zip` into `~/.cache/java-to-rust/jdk` the first time. Only the
  modules the program and its libraries use are added: java.base; the modules whose packages
  their sources name, in imports, code or strings (`Class.forName`); the modules their
  `module-info.java` files require; and, in turn, the modules those require and those that provide
  a service they use, as the JVM resolves its boot layer. `--timings` prints the modules chosen.
  The translator's patches of the JDK are made against JDK 26's sources; other versions are
  untested. With modules that the program reaches only in part, javac may stop at code of
  theirs that the patched JDK does not compile; `--keep-going` goes on past it (the code is
  rejected, and throws `NoClassDefFoundError` if it ever runs).
- `--project DIR` translates a Maven (`pom.xml`) or Gradle (`build.gradle(.kts)`,
  `settings.gradle(.kts)`) project, asking the project's own build tool what it is made of: the
  project's `mvnw` or `gradlew`, or `mvn` or `gradle` on `PATH`. The sources of every module or
  subproject are translated, together with each dependency's sources, unpacked from its sources
  jar, and the files of its jar other than classes (such as `META-INF/services`), into
  `~/.cache/java-to-rust/libraries`. Maven modules take their sources from `src/main/java` and
  their resources from `src/main/resources`; Gradle reports each project's main source set. The
  main class is the Gradle application plugin's `mainClass`, or the first `<mainClass>` of the top
  `pom.xml`; `--main` names another. `--project` implies `--jdk`. Reading the project runs its build (Maven
  compiles it and writes `target/java-to-rust-classpath.txt` in each module; Gradle configures
  it), so its build scripts run as they would for a build. The translated program reads its
  resources from the project's resource directories and the unpacked jars in the cache when it
  runs, as for any root, so it runs where those are. A dependency without a sources
  jar is listed, and code that uses it is rejected. Libraries usually reach code the translator
  rejects (often reflection or exception messages it cannot reproduce exactly); `--census` builds
  the program anyway, with rejected code throwing where it runs, and `--approximate-messages`
  accepts exception messages that may differ from the JVM's.
- `--census` translates past every rejection and reports what is rejected and why;
  `--census-json FILE` writes it as JSON. `--survey` (and `--survey-json FILE`) is a quicker
  syntactic view of what the reachable code needs.
- `--modules-json FILE` writes the module tree and what each item names
  ([docs/several-crates.md](several-crates.md)).
- `--keep-going` lets javac go on past compile errors: code that names a class missing from the
  roots throws `NoClassDefFoundError` as on the JVM, and other code with an error is rejected
  ([docs/exceptions.md](exceptions.md)). `--timings` prints the time of each phase.

