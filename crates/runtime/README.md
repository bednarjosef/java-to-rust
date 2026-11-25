# java-to-rust-runtime

The runtime library of the Rust code that [java-to-rust](https://github.com/bednarjosef/java-to-rust)
writes: Java's number semantics and printing, strings, `String.format`, regular expressions,
charsets, threads and monitors, the JDK's native methods, and the garbage collector that frees
objects. Translated programs use it as `java_runtime`; it is not meant to be used by hand.

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option, as is the translator's Java that ends up in translated programs
(`crates/translator/java/LICENSE.md`), so the translator adds no license terms of its own to what
it writes. The translator itself is GPL-3.0-or-later. A whole-program translation also contains
the JDK's code, which keeps the JDK's license.
