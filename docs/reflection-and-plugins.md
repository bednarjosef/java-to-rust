# Reflection, class loading and plugins

Java programs can find and run code by name at run time: `Class.forName`, `getMethod` and the
rest of reflection, `ServiceLoader`, class loaders, plugins and javac's annotation processors.
The translator works on a *closed world*: it sees every class of the program when it translates,
and much of what it does depends on that. This document says why the world stays closed, how
code reached by name can still run, and where the one real limit is.

Case 1 below, code that is in the program and reached by name, works except for the recording
run (see "What case 1 does now"); cases 2 and 3 are planned. A census of javac on 2026-10-21
found about 70 pieces of code in javac and the JDK parts it uses that were blocked by
reflection, class loading or `ServiceLoader`.

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

#### What case 1 does now

- **Constant names** are resolved when translating. `Class.forName("com.foo.Bar")` is that
  class, and `getName`, `getSimpleName`, `getCanonicalName`, `getPackageName`, `getSuperclass`,
  `getInterfaces`, `isAssignableFrom` and `getEnumConstants` of a class literal are worked out
  then too. A computed name becomes a `switch` over the binary names of the classes that code
  can find by name.
- **The reflection table.** `getMethod`, `getDeclaredMethod`, `getField`, `getConstructor`,
  `newInstance` and the rest look members up in a table that the translation builds in. The
  table holds only what lookups of reachable code can find. The classes are those named by
  string literals of reachable code, plus the classes of its class literals for methods and
  fields, or only the class of the receiver when it is a class literal. The member names are
  the spelled-out name, or for a computed name any string literal of reachable code, or every
  member for `getMethods()` and its relatives. Reachability finds these
  (`program/reach/reflection.rs`, kept in `Reach::reflected`). In a single file the candidates
  are its classes and the names in its string literals (`lower/reflection.rs`, `file_names`).
  `Method`, `Field` and `Constructor` are the translator's own classes
  (`java/Reflection.java`). Each holds a slot in the table, and `Method.invoke`, `Field.get` and
  `set` (with their typed forms) and `Constructor.newInstance` are a `switch` over the slots,
  each case a direct call, read or write. Arguments are checked and unboxed as Java does: a wrong
  type is an `IllegalArgumentException` and a null receiver a `NullPointerException`, and an
  exception of the called code is wrapped in an `InvocationTargetException` (by
  `Class.newInstance` it is thrown as it is, as in Java). One thing differs from Java: a final
  field is never written, even after `setAccessible(true)`; `set` throws
  `IllegalAccessException`, since the analyses rely on final fields keeping their value. Generic methods, and members
  whose types do not box to and from `Object`, are not in the table.
- **Names outside the translation** throw Java's own exception, which code can catch, with a
  message that says so: `ClassNotFoundException: com.foo.Bar, which is not in the translation`,
  and `NoSuchMethodException` or `NoSuchFieldException` for a member the table does not hold.
- **`ServiceLoader`** is the JDK's, rewritten by the translator's patch (`java/patches/java/util/ServiceLoader.java`). It
  finds the providers that the `provides` clauses of the roots' `module-info.java` files and
  their `META-INF/services` files declare, in that order (`program/modules.rs`,
  `service_providers`; kept in `Reach::services`). A provider is made lazily, by its public
  static `provider()` if it is a module's and has one, otherwise by its constructor, and is
  kept by the loader's iterators, as in Java. `loadInstalled` works the same way;
  `load(ModuleLayer, ...)` throws.
- **`ClassLoader`** is a type. The system and platform loaders exist, with `getName`,
  `getParent` and `getClassLoader()` of a class. `loadClass` goes through the `forName` table.
  `getResource`, `getResources` and `getResourceAsStream` find nothing; javac does not need them
  (its one use is `Class.getResource` behind the hidden `-XDshowClass` option).
  `URLClassLoader` is patched by the translator (`java/patches/java/net/URLClassLoader.java`). It
  finds what its parent finds and nothing of its own. A class or resource that is in one of its
  directories, or a class asked of it while it has a jar file among its URLs, fails loudly:
  "code that is not in the translation cannot run". A `ServiceLoader` over it fails the same
  way when a directory declares providers of the service. Defining classes from bytes stays
  rejected.
- **javac without plugins**: javac always looks for `Plugin`s and annotation `Processor`s with a
  `ServiceLoader` over a `URLClassLoader` of its class path. When the class path has no
  providers (no `-processor`, `-processorpath` or `-Xplugin`, and no `META-INF/services` files
  for them), discovery finds nothing, and the program is not rejected. When it does have
  providers, javac reports its own error, "Bad service configuration file", with our message
  naming the directory.

- **Modules.** With the JDK's sources, `java.lang.Module` and `ModuleLayer` are the translator's
  own (`java/overlay/java/lang/`), over what the roots declare and hold
  (`program/modules.rs`, `contents`; kept in `Reach::modules`, and given to `Module` by its
  natives, `lower/modules.rs`). Each root with a `module-info.java` is a named module, with the
  class loader that the JVM gives it (the boot loader for `java.base` and the JDK's other boot
  modules, the platform loader for `java.compiler` and the rest of the platform's, the system
  loader for any other), and the roots without one are the unnamed module of the system class
  loader. A class is in the module whose root has its package, and the translator's own classes
  are `java.base`'s. A module answers `isNamed`, `getName`, `getClassLoader`, `canRead`,
  `isExported`, `isOpen`, `canUse` and `getPackages` from its declaration, and `addReads`,
  `addExports`, `addOpens` and `addUses` add to that as in Java; `ModuleLayer.boot()` is the
  layer of the named modules, over the empty layer. `Class.getModule()`,
  `Class.getClassLoader()` and `ClassLoader.getUnnamedModule()` go to the translator's
  `TranslatedModules`, and so do the JDK's `BootLoader.getUnnamedModule`, `loadClassOrNull`
  and `findResource`, `ClassLoaders`' loaders and `Reflection.verifyModuleAccess`
  (`REDIRECTED_CALLS`). `Class.forName(module, name)` finds the class only in that module.
  `ServiceLoader` checks, as Java does, that a named module that loads a service declares that it
  uses it. What would need code or data from outside the translation throws
  `UnsupportedOperationException` when it runs, saying "code that is not in the translation
  cannot run": defining a module layer, the boot layer's `configuration()` (which javac asks for
  only with `--processor-module-path`), a module's descriptor and annotations, and a resource
  that a root holds. Two differences from Java: `addExports` and the other `add` methods do not
  check that their caller is in the module, and the boot layer holds only the translation's
  named modules, where the JVM's also holds the system modules that service binding resolves.

#### javac's survey, before and after

`--survey` of javac (jdk.compiler with java.compiler, jdk.internal.opt and java.base), pieces of
code that use each feature, on 2026-10-22: before case 1, with the reflection table, and with the
modules.

| feature       | javac's code before | table | modules | the JDK's code before | table | modules |
|---------------|--------------------:|------:|--------:|----------------------:|------:|--------:|
| reflection    |                   4 |     0 |       0 |                    53 |    41 |      39 |
| class loading |                  11 |     4 |       0 |                    30 |    16 |       4 |

The four of javac that the table left used the module system on paths that only code outside
the translation takes: `BasicJavacTask.initPlugin` once a plugin is found,
`JavacFileManager.getServiceLoader` for a processor module path (`--processor-module-path`), the
`accessInternalAPI` option, and the source launcher's `ModuleLayer.boot()`. They now translate,
and those paths fail when they run, saying so. The four left in the JDK's code are
`BootLoader.loadLibrary`, which the lowering makes a call that does nothing
(`lower/libraries.rs`). In the JDK's code, the reflection left is mostly annotations and method
handles.

#### What is left of case 1

- **The recording run.** Names that code computes at run time from data the translator does not
  see (a configuration file, a command-line option) are found only by chance today: as some
  string literal of the program. The recording run would plug in where the lookups' candidates
  are chosen: its record would add classes and member names to `Walk::reflect` (and so to
  `Reach::reflected` and `Reach::services`) in a whole program, and to `file_names` in a single
  file. The rest of the table needs nothing new.
- **The report** of the lookups that cannot be resolved when translating is not written yet.
- **Annotations read by reflection** (`getAnnotation`, javac's `AnnotationProxyMaker`), method
  handles, `Proxy` and `java.lang.invoke` are still rejected, except the three annotations that
  `AbstractProcessor` reads of its processor's class (`@SupportedOptions`,
  `@SupportedAnnotationTypes`, `@SupportedSourceVersion`): its overlay reads them through the
  native `TranslatedProcessorAnnotations.values0`, which the translator writes from the program's
  annotations (`lower/processors.rs`).
- **Module layers** other than the boot layer, which load code from a module path, and a
  module's `ModuleDescriptor`.
- **A bundle of the program's own** (`ResourceBundle.getBundle` of a class of the program, as
  javac's messages are) translates in a named module: a class bundle through `ServiceLoader` and
  `Class.forName(module, name)`, and a properties file through `PropertyResourceBundle`, whose
  overlay decodes the bytes as Java's decoder does (UTF-8, or ISO-8859-1 from the start when the
  first malformed byte is in the first 8192 bytes; a character that the end cuts is a
  `MalformedInputException`). What it cannot match, a malformed byte later or the property
  `java.util.PropertyResourceBundle.encoding` set to `UTF-8` with malformed input, throws
  `UnsupportedOperationException`. The JDK's `ResourceBundle` keeps any exception of loading a
  bundle as the cause of the `MissingResourceException` it may throw (`catch (LinkageError|Exception
  e)` in `loadBundle`), so the message of every `NullPointerException` that the code under it
  could raise must be known (see `docs/exceptions.md`). A program that loads a properties file
  still does not compile: `Properties.entrySet()` is a `Collections.synchronizedSet` over the
  `Properties` as its lock, and the generator gives no monitor to an `Object` that may hold a
  value such as a string.
- **Members of the runtime's classes** (`String`, the boxes, `Object`, ...) are not in the
  table: a lookup on `String.class` finds nothing. `Modifier` is not translated in a single file.

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
   reflection) and most ordinary programs. Done except for the recording run, the report,
   annotations read by reflection and module layers (see "What is left of case 1").
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
