# Reflection, class loading and plugins (planned)

Java programs can find and run code by name at run time: `Class.forName`, `getMethod` and the
rest of reflection, `ServiceLoader`, class loaders, plugins and javac's annotation processors.
The translator works on a *closed world*: it sees every class of the program when it translates,
and much of what it does depends on that. This document says why the world stays closed, how
code reached by name can still run, and where the one real limit is.

Today all of this is rejected ("(reflection) not supported yet", "type `java.lang.ClassLoader`
not supported yet"). A census of javac on 2026-10-21 found about 70 pieces of code in javac and
the JDK parts it uses that are blocked by reflection, class loading or `ServiceLoader`.

## Why the world stays closed

Because the translator sees every class that will ever exist, it can prove things, and the proofs
are what make the output good:

- **Class hierarchies become Rust enums.** A virtual call is a `match` over the known subclasses
  ([object-model.md](object-model.md)).
- **The analyses hold for the whole program**: a value that is never null needs no `Option`, an
  object with one owner needs no `Rc`, a method that never throws needs no `Result`, arithmetic
  that cannot overflow needs no wrapping ([thorough-mode.md](thorough-mode.md),
  [natural-rust.md](natural-rust.md)).
- **Unreachable code is left out**, so a program that uses a `HashMap` does not carry the JDK.
- The goal of a translated javac that uses **less memory than on the JVM**
  ([translator-speed-and-memory.md](translator-speed-and-memory.md)) rests on these proofs.

An open world, where any class can be subclassed, any method overridden and any field read or
written by code that appears at run time, takes all of that away: every call goes through a
table, every value may be null, every object needs shared ownership and a lock, and nothing can be
left out. The output becomes a slow imitation of a JVM, and likely slower than the JVM itself,
which has a just-in-time compiler to win the speed back.

And it would still not run code that arrives at run time: a plugin jar is Java bytecode, and
running it takes an interpreter or a just-in-time compiler, that is, a JVM. Every
ahead-of-time compiler for Java meets this. GraalVM's native-image is closed-world for the same
reason; Android, which needs both, ships a full VM with an interpreter and a just-in-time compiler
next to its precompiled code.

So the plan keeps the world closed, and makes code that is reached by name work inside it.

## Three cases, by when the code becomes known

### 1. The code is in the program, and is reached by name

`Class.forName("com.foo.Bar")`, `getMethod("run")`, reading a field by name, `ServiceLoader`
finding the implementations of an interface, resource bundles, annotations read by reflection,
`Enum.valueOf`. The code is in the sources, so the translator has seen it; what is missing is the
table the JVM keeps to find it by name.

The translation builds that table in, for the classes reflection can reach and only for them:
their names, constructors, methods and fields, each with a function that calls it or reads it.
Which classes those are comes from three places:

- **Constant names** (`Class.forName("com.foo.Bar")`, `X.class.getMethod("run")`) are resolved
  when translating; the call becomes a direct call and needs no table at all.
- **Fixed lists**: `META-INF/services` files and the `provides` clauses of `module-info.java` say
  which classes `ServiceLoader` finds; the translator reads them.
- **Names computed at run time** (from a configuration file, a command-line option, a string
  built in code) are found by a **recording run**: the program runs once on the JVM with a small
  agent that records every class, method and field looked up by name, and the translation takes
  that record as an input. The corpus and target checks already run every program on Java, so
  they can record at the same time. GraalVM's native-image works the same way (its tracing agent
  and reachability metadata).

The classes in the table count as reached, and the analyses treat what reflection can do to them
as possible: a method in the table may be called with any arguments, a field in the table may be
written. Everything outside the table keeps its proofs.

A name that is in no table at run time raises a clear error that names the class or member and
says the recording run did not see it, and the translator's report lists every place in the
program where such a lookup can happen, so they can be checked before the program ships.

### 2. Plugins and processors chosen when translating

javac with Lombok as an annotation processor, an application with three known plugins. The
plugins' sources (or a decompiled jar's) are passed to the translator with the program, as one
more source root, and become part of the translation like any other code; the table of case 1
lets the program find them by name as it does on the JVM. This is also how Rust programs usually
take plugins: chosen when building.

Needs: the translator reading several source roots and modules
([large-projects.md](large-projects.md)), and case 1.

### 3. Code that arrives at run time

A user drops a new plugin jar into a folder of the running program. A native binary cannot run
Java code it has never seen. There are two ways, and they can be combined:

- **Open doors at chosen interfaces.** The interfaces that plugins implement (javac's `Processor`
  and `Plugin`, an application's extension points) are translated as open Rust traits instead of
  closed enums, and the program can load native libraries that implement them. A plugin is then
  translated on its own, against the program's description of those interfaces, into a library
  that the translated program loads at run time without being rebuilt. Only the doors pay the
  open-world cost: calls through them go through a table, and values passed through them are
  assumed nullable and shared. Everything else keeps its proofs.
- **An embedded JVM as the last resort**, for plugin jars that nobody translated. The translated
  program starts a JVM only when such code appears, and passes objects back and forth through a
  bridge (proxies on both sides, through JNI). This runs everything, but that part runs at the
  JVM's speed and with its memory, and the bridge is a large piece of work of its own.

## Order

1. **Case 1**: the reflection table, constant names resolved when translating, `ServiceLoader`
   from the fixed lists, the recording run, the error and the report for names outside the table.
   This covers what javac needs (its `ServiceLoader` uses, `Class.cast`, the annotation
   reflection) and most ordinary programs. About a day of agent time.
2. **Case 2** comes nearly free once the translator reads several source roots and modules, which
   the javac work needs anyway. With cases 1 and 2, a translated javac runs fully for normal use,
   with annotation processors named when translating.
3. **Open doors** (case 3, first way) when a target needs plugins loaded at run time; it builds on
   the generator producing items with module paths and the crate layering
   ([architecture.md](architecture.md), [large-projects.md](large-projects.md)).
4. **The embedded JVM** only if a target needs code that nobody translated, Minecraft's mods being
   the classic example. Until then, this is the translator's documented limit: code that is not
   in the translation and not behind a door does not run.

## Limits

- A translated program runs only the code it was translated with, plus native libraries loaded
  through its doors, plus (if built) the code its embedded JVM runs.
- The recording run sees only what that run does: a name looked up only on a path it did not
  take is missing from the table. The report lists every lookup that cannot be resolved when
  translating, so these can be covered by a broader run or by naming the classes by hand.
- Reflection that changes what the program is (defining classes from bytes, `MethodHandles.Lookup`
  defining hidden classes, instrumentation agents) is out of scope except through the embedded
  JVM.
