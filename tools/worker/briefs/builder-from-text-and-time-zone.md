# StringBuilder from a CharSequence, and the system time zone

Whole-program translations of `targets/ScannerGrades.java` reach two small gaps:
`new StringBuilder(CharSequence)` (from `jdk/internal/icu/text/FilteredNormalizer2.java:243`) and
the natives `TimeZone.getSystemTimeZoneID(String javaHome)` and `TimeZone.getSystemGMTOffsetID()`.

First read how `StringBuilder`'s existing constructors are recognised and generated (search
`crates/translator/src` for "this constructor of"), and how an existing native goes from
recognition to runtime function to test (`lower/natives.rs`, `ast/natives.rs`,
`codegen/natives.rs`, the runtime's natives; for example `Native::NanoTimeAdjustment`).

## Scope
- `new StringBuilder(CharSequence seq)` and `new StringBuffer(CharSequence seq)`: the builder
  starts with `seq`'s characters; a null `seq` throws NullPointerException as Java does. If the
  CharSequence may be a program class, follow what the recent `append(CharSequence, int, int)`
  work does (it rejects with "appending a range of a `CharSequence` that a program class
  implements"): reject the same way rather than guess.
- `TimeZone.getSystemTimeZoneID(javaHome)`: Java's Linux logic: the `TZ` environment variable if
  set (without a leading `:`), else `/etc/timezone`'s first line, else the zone name that the
  `/etc/localtime` symbolic link points to (the part after `zoneinfo/`), else null. Read the
  JDK's native source behaviour from its Java callers and the JDK documentation; when unsure,
  compare with what `java` prints for `TimeZone.getDefault().getID()` on this machine, also with
  `TZ=Europe/Prague` and `TZ=UTC` set.
- `TimeZone.getSystemGMTOffsetID()`: the "GMT+hh:mm" form of the local offset, as Java does.
- Then try `TimeZone.getDefault().getID()` in a whole program and report how far it gets (the
  JDK then reads `lib/tzdb.dat` under `java.home`). Make it work only if what is left is small
  and clearly correct; otherwise list what stops it.

Each must behave exactly as Java. Anything you cannot do correctly stays rejected; say so in your
report rather than guessing.

## Deliverables
- new StringBuilder(CharSequence) and new StringBuffer(CharSequence)
- TimeZone.getSystemTimeZoneID and getSystemGMTOffsetID natives
- One corpus program corpus/features/BuilderFromTextAndZone.java (builders from a String and a
  StringBuilder, a caught NullPointerException's message; the zone only if the corpus can
  compare it with Java, which it can when both run on this machine)
- A report of what `TimeZone.getDefault()` still needs

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-to-rust --test corpus`.
  `CORPUS_FILTER=builderfromtext` runs only programs whose lowercase name contains it.
- Whole programs: `JAVA_BASE_SOURCES=<scratchpad>/jdk-src/java.base`
  with `cargo run -q -p java-to-rust -- --main Name --source DIR --library $JAVA_BASE_SOURCES --crate OUT`
  (DIR holds only your program), through the limiter.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, file system, reflection, ClassLoader, Appendable or
  enum code: other agents are changing it now.
