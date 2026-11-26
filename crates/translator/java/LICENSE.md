# Licensing of the Java that becomes part of translated programs

The translator is GPL-3.0-or-later (see [LICENSE](../../../LICENSE)). These files of this
directory are translated into the programs the translator writes, so, like the runtime crate,
they are licensed under either of the [Apache License, Version 2.0](../../runtime/LICENSE-APACHE)
or the [MIT license](../../runtime/LICENSE-MIT), at your option:

- `overlay/`: the translator's classes that replace or add to the JDK's;
- `patches/`: the translator's edits of the JDK's classes. Where a patch quotes or adapts the
  JDK's code (the texts that `@Edit` finds and replaces, a member that `@Replace` rewrites from
  the JDK's), that code keeps the JDK's license (GPL-2.0 with the Classpath Exception for
  OpenJDK); what the translator adds is MIT OR Apache-2.0;
- `Standard.java`, `Functional.java`, `Threads.java`, `ThreadLocalStub.java` and `Reflection.java`.

The other files here (`JavaTree.java`, `BytecodeNames.java`, `JdkPatch.java`, `MissingClasses.java`,
`Charsets.java`, `JdkResources.java`) are part of the translator and are GPL-3.0-or-later.

A whole-program translation also contains the JDK's own code, translated: that code keeps the
JDK's license (GPL-2.0 with the Classpath Exception for OpenJDK).
