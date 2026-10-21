# JDK coverage

How much of the JDK's code that typical programs reach the translator translates today. Measured on 2026-10-21 at commit `9634999` (on main) by `scripts/jdk-coverage.py`, which regenerates this file, over the 28 programs of `targets/` and the driver `targets/driver/JdkApiDriver.java`, each translated whole with the sources of `java.base`.

## Method

`java-to-rust --census-json` (`program/census.rs`) runs the translator's real pipeline (lowering, the analyses and the generator) over everything reachable from `main`, without stopping at the first rejection. Lowering records each rejected method, constructor or lambda and lowers a stub in its place, with its signature and a body that never runs; a rejected field initializer or initializer block is left out. The errors of an analysis or of the generator stub the methods at their positions, and they run again (the generator and the analysis of exceptions report every error they find at once, the other analyses their first). So every rejection counted is one that the translation would report once the code before it translated. A *piece* is a reachable method, constructor, field initializer or initializer block, as `--survey` counts them, with the lambdas, local and anonymous classes it declares; it translates when the census records no rejection in it. Messages are grouped with the names in backquotes left out (`type \`…\` not supported yet`).

Limits:

- A piece counts as translating when the census records no rejection in it. It may still call a piece that does not, and its translation was neither compiled nor run.
- A piece is stubbed at its first rejection in each phase, so a piece with several blockers may show only some of them; the counts of pieces blocked are exact, the lists of blockers per piece are a lower bound.
- An error of an analysis or of the generator has a line and column but no file. The census stubs the code at that position in every file, so on rare collisions it also hides the code of an innocent method from the later phases.
- A rejection that no stub can take away ends the census early, and so does a panic of the translator outside the generator; the numbers then count only what was checked before the stop. A rejection of a class's declaration, or in code that lowering provides, belongs to no piece; it is listed in the census as rejected elsewhere.
- The analyses run again after each round of stubs. Most programs take seconds, but those that reach `Scanner` (regular expressions, `java.time`; about 9000 pieces, where one run of the analyses takes minutes) take most of an hour or more.
- Incomplete censuses: ScannerGrades, TextAdventure (see the per-program table).

## Headline

- Library code reached by the 28 targets (union): 9168 / 9514 (96.4%) pieces translate; methods and constructors alone: 7243 / 7567 (95.7%).
- Library code reached by the driver: 2215 / 2229 (99.4%) pieces translate; methods and constructors: 2013 / 2026 (99.4%).
- The targets' own code: 467 / 470 (99.4%) pieces translate.
- Programs with no rejection at all: 24 of 28 (AnimalsInterfaces, BoundedGenerics, CheckedExceptions, ComparatorSorting, DequeBfs, DocumentVisitor, EnumAbstractMethods, ExpressionTree, GenericContainers, InnerClassIterator, IntegerKeyedGroups, InventoryManagement, JsonPrettyPrinter, LambdaFunctions, ListOperations, LruCacheLinkedHashMap, PriorityScheduler, RecordsAndSealed, ShapesAbstract, StringBuilderTable, TokenizerParser, TryWithResources, UncaughtException, WordFrequency).

## Per program

| Program | Own pieces translate | Library pieces translate | Distinct rejections | Complete |
|---|---:|---:|---:|---|
| AnimalsInterfaces | 24 / 24 | 228 / 228 (100.0%) | 0 | yes |
| BoundedGenerics | 12 / 12 | 319 / 319 (100.0%) | 0 | yes |
| CheckedExceptions | 12 / 12 | 0 / 0 (-) | 0 | yes |
| ComparatorSorting | 9 / 9 | 502 / 502 (100.0%) | 0 | yes |
| DequeBfs | 10 / 10 | 94 / 94 (100.0%) | 0 | yes |
| DocumentVisitor | 29 / 29 | 228 / 228 (100.0%) | 0 | yes |
| EnumAbstractMethods | 22 / 22 | 203 / 203 (100.0%) | 0 | yes |
| EventListeners | 23 / 24 | 349 / 349 (100.0%) | 1 | yes |
| ExpressionTree | 39 / 39 | 95 / 95 (100.0%) | 0 | yes |
| GenericContainers | 23 / 23 | 49 / 49 (100.0%) | 0 | yes |
| InnerClassIterator | 14 / 14 | 255 / 255 (100.0%) | 0 | yes |
| IntegerKeyedGroups | 4 / 4 | 372 / 372 (100.0%) | 0 | yes |
| InventoryManagement | 29 / 29 | 393 / 393 (100.0%) | 0 | yes |
| JsonPrettyPrinter | 24 / 24 | 348 / 348 (100.0%) | 0 | yes |
| LambdaFunctions | 5 / 5 | 413 / 413 (100.0%) | 0 | yes |
| ListOperations | 3 / 3 | 550 / 550 (100.0%) | 0 | yes |
| LruCacheLinkedHashMap | 15 / 15 | 130 / 130 (100.0%) | 0 | yes |
| PriorityScheduler | 8 / 8 | 263 / 263 (100.0%) | 0 | yes |
| RecordsAndSealed | 37 / 37 | 288 / 288 (100.0%) | 0 | yes |
| ScannerGrades | 5 / 6 | 8875 / 9212 (96.3%) | 44 | no: a `StringBuilder` that threads may share not supported yet (java/util/regex/Pattern.java:1670:36) |
| ShapesAbstract | 21 / 21 | 228 / 228 (100.0%) | 0 | yes |
| StreamPipeline | 11 / 11 | 1257 / 1261 (99.7%) | 4 | yes |
| StringBuilderTable | 12 / 12 | 437 / 437 (100.0%) | 0 | yes |
| TextAdventure | 14 / 15 | 8860 / 9196 (96.3%) | 44 | no: a `StringBuilder` that threads may share not supported yet (java/util/regex/Pattern.java:1670:36) |
| TokenizerParser | 33 / 33 | 343 / 343 (100.0%) | 0 | yes |
| TryWithResources | 14 / 14 | 246 / 246 (100.0%) | 0 | yes |
| UncaughtException | 10 / 10 | 109 / 109 (100.0%) | 0 | yes |
| WordFrequency | 5 / 5 | 250 / 250 (100.0%) | 0 | yes |
| JdkApiDriver | 22 / 23 | 2215 / 2229 (99.4%) | 10 | yes |

## Rejections per program

Each program's rejections, grouped by message, with the pieces each blocks (up to 15; the programs with none are left out).

### EventListeners

| Pieces | Rejection |
|---:|---|
| 1 | the message of a `NullPointerException` that the runtime raises not supported yet |

### ScannerGrades

| Pieces | Rejection |
|---:|---|
| 106 | method `…` not supported yet |
| 45 | `…` with an offset that is not a field's not supported yet |
| 24 | type `…` not supported yet |
| 21 | native method `…` not supported yet |
| 17 | fields not supported yet |
| 16 | `…` (reflection) not supported yet |
| 15 | `…` not supported yet |
| 12 | `==` on `Object` values that may be strings or boxes not supported yet |
| 10 | the name `Temporal` here not supported yet |
| 8 | `…`: the JVM sets this value, which the translation does not give not supported yet |
| 8 | this overload of `…` not supported yet |
| 6 | a call of `objectFieldOffset` that lowering left unresolved |
| 5 | instanceof on these types not supported yet |
| 4 | a call of `loadInstalled` that lowering left unresolved |
| 3 | malformed javac tree: IDENTIFIER without expression |
| | and 29 more |

### StreamPipeline

| Pieces | Rejection |
|---:|---|
| 1 | a local of type `Object` declared without a value and assigned in a `try` block not supported yet |
| 1 | a static initializer that throws inside a `try` not supported yet |
| 1 | a toString that throws an exception that is caught not supported yet |
| 1 | catching an exception that this call of the library may throw not supported yet |

### TextAdventure

| Pieces | Rejection |
|---:|---|
| 106 | method `…` not supported yet |
| 45 | `…` with an offset that is not a field's not supported yet |
| 24 | type `…` not supported yet |
| 21 | native method `…` not supported yet |
| 16 | `…` (reflection) not supported yet |
| 16 | fields not supported yet |
| 15 | `…` not supported yet |
| 12 | `==` on `Object` values that may be strings or boxes not supported yet |
| 10 | the name `Temporal` here not supported yet |
| 8 | `…`: the JVM sets this value, which the translation does not give not supported yet |
| 8 | this overload of `…` not supported yet |
| 6 | a call of `objectFieldOffset` that lowering left unresolved |
| 5 | instanceof on these types not supported yet |
| 4 | a call of `loadInstalled` that lowering left unresolved |
| 3 | malformed javac tree: IDENTIFIER without expression |
| | and 29 more |

### JdkApiDriver

| Pieces | Rejection |
|---:|---|
| 4 | catching an exception that this call of the library may throw not supported yet |
| 2 | `==` on `Object` values that may be strings or boxes not supported yet |
| 2 | a local of type `Object` declared without a value and assigned in a `try` block not supported yet |
| 1 | `…` (reflection) not supported yet |
| 1 | a change in place of a `…` that is also an `…` not supported yet |
| 1 | a local of type `…` declared without a value and assigned in a `try` block not supported yet |
| 1 | a static initializer that throws inside a `try` not supported yet |
| 1 | a toString that throws an exception that is caught not supported yet |
| 1 | catching the exception of this operation not supported yet |
| 1 | native method `…` not supported yet |

## Top 15 rejections over the targets, by pieces they block

Pieces are counted once however many programs reach them; program and library code together.

| Rejection | Pieces | Programs | Most common forms |
|---|---:|---:|---|
| method `…` not supported yet | 106 | 2 | method `Character.codePointAt` not supported yet (16); method `CharSequence.subSequence` not supported yet (12); method `StringBuilder.repeat` not supported yet (8) |
| `…` with an offset that is not a field's not supported yet | 45 | 2 | `Unsafe.getReferenceAcquire` with an offset that is not a field's not supported yet (7); `Unsafe.putReference` with an offset that is not a field's not supported yet (4); `Unsafe.compareAndSetReference` with an offset that is not a field's not supported yet (3) |
| type `…` not supported yet | 24 | 2 | type `java.lang.ClassLoader` not supported yet (7); type `java.lang.Thread.UncaughtExceptionHandler` not supported yet (6); type `java.lang.invoke.MethodHandles.Lookup` not supported yet (4) |
| native method `…` not supported yet | 21 | 2 | native method `jdk.internal.misc.Unsafe.unpark(java.lang.Object)void` not supported yet (4); native method `java.io.UnixFileSystem.getBooleanAttributes0(java.io.File)int` not supported yet (2); native method `jdk.internal.misc.Unsafe.park(boolean,long)void` not supported yet (2) |
| fields not supported yet | 18 | 2 |  |
| `…` (reflection) not supported yet | 16 | 2 | `Class.getCanonicalName` (reflection) not supported yet (3); `Class.getPackageName` (reflection) not supported yet (3); `Class.getDeclaredField` (reflection) not supported yet (2) |
| `==` on `Object` values that may be strings or boxes not supported yet | 15 | 2 |  |
| `…` not supported yet | 15 | 2 | `Thread.isVirtual` not supported yet (7); `Thread.getThreadGroup` not supported yet (3); `Throwable.initCause` not supported yet (2) |
| the name `Temporal` here not supported yet | 10 | 2 |  |
| `…`: the JVM sets this value, which the translation does not give not supported yet | 8 | 2 | `System.getProperty("java.locale.providers")`: the JVM sets this value, which the translation does not give not supported yet (1); `System.getProperty("java.time.zone.DefaultZoneRulesProvider")`: the JVM sets this value, which the translation does not give not supported yet (1); `System.getProperty("jdk.calendar.japanese.supplemental.era")`: the JVM sets this value, which the translation does not give not supported yet (1) |
| this overload of `…` not supported yet | 8 | 2 |  |
| a call of `objectFieldOffset` that lowering left unresolved | 6 | 2 |  |
| a call of `loadInstalled` that lowering left unresolved | 5 | 2 |  |
| instanceof on these types not supported yet | 5 | 2 |  |
| malformed javac tree: IDENTIFIER without expression | 3 | 2 |  |

## Top 15 rejections over the targets, by programs they appear in

| Rejection | Pieces | Programs | Most common forms |
|---|---:|---:|---|
| method `…` not supported yet | 106 | 2 | method `Character.codePointAt` not supported yet (16); method `CharSequence.subSequence` not supported yet (12); method `StringBuilder.repeat` not supported yet (8) |
| `…` with an offset that is not a field's not supported yet | 45 | 2 | `Unsafe.getReferenceAcquire` with an offset that is not a field's not supported yet (7); `Unsafe.putReference` with an offset that is not a field's not supported yet (4); `Unsafe.compareAndSetReference` with an offset that is not a field's not supported yet (3) |
| type `…` not supported yet | 24 | 2 | type `java.lang.ClassLoader` not supported yet (7); type `java.lang.Thread.UncaughtExceptionHandler` not supported yet (6); type `java.lang.invoke.MethodHandles.Lookup` not supported yet (4) |
| native method `…` not supported yet | 21 | 2 | native method `jdk.internal.misc.Unsafe.unpark(java.lang.Object)void` not supported yet (4); native method `java.io.UnixFileSystem.getBooleanAttributes0(java.io.File)int` not supported yet (2); native method `jdk.internal.misc.Unsafe.park(boolean,long)void` not supported yet (2) |
| fields not supported yet | 18 | 2 |  |
| `…` (reflection) not supported yet | 16 | 2 | `Class.getCanonicalName` (reflection) not supported yet (3); `Class.getPackageName` (reflection) not supported yet (3); `Class.getDeclaredField` (reflection) not supported yet (2) |
| `==` on `Object` values that may be strings or boxes not supported yet | 15 | 2 |  |
| `…` not supported yet | 15 | 2 | `Thread.isVirtual` not supported yet (7); `Thread.getThreadGroup` not supported yet (3); `Throwable.initCause` not supported yet (2) |
| the name `Temporal` here not supported yet | 10 | 2 |  |
| `…`: the JVM sets this value, which the translation does not give not supported yet | 8 | 2 | `System.getProperty("java.locale.providers")`: the JVM sets this value, which the translation does not give not supported yet (1); `System.getProperty("java.time.zone.DefaultZoneRulesProvider")`: the JVM sets this value, which the translation does not give not supported yet (1); `System.getProperty("jdk.calendar.japanese.supplemental.era")`: the JVM sets this value, which the translation does not give not supported yet (1) |
| this overload of `…` not supported yet | 8 | 2 |  |
| a call of `objectFieldOffset` that lowering left unresolved | 6 | 2 |  |
| a call of `loadInstalled` that lowering left unresolved | 5 | 2 |  |
| instanceof on these types not supported yet | 5 | 2 |  |
| malformed javac tree: IDENTIFIER without expression | 3 | 2 |  |

## Top 15 rejections in the driver, by pieces they block

| Rejection | Pieces | Programs | Most common forms |
|---|---:|---:|---|
| catching an exception that this call of the library may throw not supported yet | 4 | 1 |  |
| `==` on `Object` values that may be strings or boxes not supported yet | 2 | 1 |  |
| a local of type `Object` declared without a value and assigned in a `try` block not supported yet | 2 | 1 |  |
| `…` (reflection) not supported yet | 1 | 1 |  |
| a change in place of a `…` that is also an `…` not supported yet | 1 | 1 |  |
| a local of type `…` declared without a value and assigned in a `try` block not supported yet | 1 | 1 |  |
| a static initializer that throws inside a `try` not supported yet | 1 | 1 |  |
| a toString that throws an exception that is caught not supported yet | 1 | 1 |  |
| catching the exception of this operation not supported yet | 1 | 1 |  |
| native method `…` not supported yet | 1 | 1 |  |

## Natives

The programs reach 44 native methods; the runtime has 26 of them. Missing, with the number of programs (driver included) that reach each:

| Native method | Programs |
|---|---:|
| `jdk.internal.misc.Unsafe.objectFieldOffset0(java.lang.reflect.Field)long` | 3 |
| `java.io.FileCleanable.cleanupClose0(int,long)void` | 2 |
| `java.io.UnixFileSystem.getBooleanAttributes0(java.io.File)int` | 2 |
| `java.lang.StackStreamFactory.AbstractStackWalker.callStackWalk(int,int,jdk.internal.vm.ContinuationScope,jdk.internal.vm.Continuation,int,int,T[])R` | 2 |
| `java.lang.StackStreamFactory.AbstractStackWalker.fetchStackFrames(int,long,int,int,int,T[])int` | 2 |
| `java.lang.StackStreamFactory.AbstractStackWalker.setContinuation(long,T[],jdk.internal.vm.Continuation)void` | 2 |
| `java.lang.StackStreamFactory.checkStackWalkModes()boolean` | 2 |
| `java.lang.VirtualThread.takeVirtualThreadListToUnblock()java.lang.VirtualThread` | 2 |
| `java.lang.reflect.Array.newArray(java.lang.Class<?>,int)java.lang.Object` | 2 |
| `java.util.TimeZone.getSystemGMTOffsetID()java.lang.String` | 2 |
| `java.util.TimeZone.getSystemTimeZoneID(java.lang.String)java.lang.String` | 2 |
| `jdk.internal.misc.Unsafe.copyMemory0(java.lang.Object,long,java.lang.Object,long,long)void` | 2 |
| `jdk.internal.misc.Unsafe.copySwapMemory0(java.lang.Object,long,java.lang.Object,long,long,long)void` | 2 |
| `jdk.internal.misc.Unsafe.fullFence()void` | 2 |
| `jdk.internal.misc.Unsafe.park(boolean,long)void` | 2 |
| `jdk.internal.misc.Unsafe.staticFieldBase0(java.lang.reflect.Field)java.lang.Object` | 2 |
| `jdk.internal.misc.Unsafe.staticFieldOffset0(java.lang.reflect.Field)long` | 2 |
| `jdk.internal.misc.Unsafe.unpark(java.lang.Object)void` | 2 |
