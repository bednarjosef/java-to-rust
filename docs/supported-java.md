# Supported Java

What the translator accepts today, what Rust each construct becomes, and what is still rejected.
Anything outside the supported subset is rejected at translation time with an error that names
the construct and its position; the translator never writes Rust it knows to be wrong. The
design behind each area is in its own document, linked from the section.

## Two modes

- **A single file** (`java-to-rust Program.java`): the program's own classes, plus what the
  runtime crate supplies by hand (strings, builders, the boxes, `Math`, `System`, the standard
  exceptions, `Thread`). Any other library class, such as `java.util.List`, is rejected
  ("type `java.util.List` not supported yet").
- **A whole program** (`--main Main --source src/ --library <java.base sources>`): the JDK's own
  code is translated with the program. Reachability (below) picks the classes, methods and
  fields that `main` can reach, so collections, streams, `java.util.concurrent`, `java.io`,
  `java.nio.file` and the rest come from the JDK's sources. `corpus/whole-program/` holds the
  programs that show what works.

## Pipeline

javac parses and attributes the source (no code generation) through
`crates/translator/java/JavaTree.java`, which prints javac's tree as JSON with the type of every
expression and the declaration every name and call refers to. `lower.rs` maps that tree onto the
supported subset (`ast.rs`); the analyses prove facts about it (`analysis.rs`); the generator
writes Rust (`codegen.rs`) that uses the runtime crate (`crates/runtime`). Only a program that
reads the message of a `NullPointerException` is compiled a second time, with code generation,
by `java/BytecodeNames.java`, for the names that the message takes from the class file. See
[architecture.md](architecture.md).

## Representations: faithful first, plainer where proven

Every value starts in a faithful form that is always correct, and an analysis moves it to a
plainer form only where it proves the meaning is the same:

| Java | Faithful form | Plainer forms, where an analysis allows |
|---|---|---|
| object | `Arc<RefCell<C>>` | `Arc<C>` when objects never change after construction (`analysis/objects.rs`); a plain struct with `&self`/`&mut self` methods when objects are never shared (`analysis/owned.rs`, `analysis/borrowing.rs`); made in a slot on its method's stack instead of the heap when it never leaves that method (`analysis/escape.rs`) |
| reference that may be null | `Option<..>` | the plain value where nullness analysis proves it is never null (`analysis/nullness.rs`) |
| `int` arithmetic | `wrapping_add`, `java::rem`, ... | `+ - * / %` where range analysis proves no overflow and a nonzero divisor (`analysis/ranges.rs`) |
| array | `java::Array<T>`, a shared cell | `Vec<T>` (or `Vec<Vec<T>>`) when no other name can see it (`analysis/arrays.rs`) |
| string | `java::JString`, WTF-8 bytes, whether they are all ASCII, and the hash once computed | `&java::Wtf8` for a parameter never assigned (`java::Str`, its text and whether it is ASCII, where the method indexes it); byte indexing for strings proven ASCII, or kept as ASCII |
| object in a threaded program | `Arc<java::Monitored<C>>` of `java::SharedField`s | `Arc<C>` when immutable; `Arc<C>` of `java::Field`s when no other thread can reach it; no `java::Monitored` when no monitor site can lock it |

Nullness analysis treats a field as not null when every constructor sets it to a non-null value
before anything can read it. A static read while its own class's initialization is still under
way, before the initializer sets it, may be null wherever its value goes
(`analysis/early_reads.rs`).

`Arc` is `java::Arc`, a handle with no count. Objects are freed by a garbage collector, cycles
included (docs/collector.md).

## Primitives, statements and operators

- All eight primitive types and `String`. `byte`, `short`, `char` and `float` are `i8`, `i16`,
  `u16` and `f32`; Java's `char` is a UTF-16 code unit, not Rust's `char`. The small types are
  promoted to `int` for arithmetic (`c as i32 - 'a' as i32`), and `b += x` and `b++` on them
  narrow with wrapping arithmetic in the small type.
- `if`, `while`, `do`, `for`, for-each over arrays, labelled `break` and `continue`, the
  conditional operator, `switch` statements and expressions on integers, chars, strings and enums
  (case labels may be constant expressions, folded as javac folds them; an `Integer` or
  `Character` selector is unboxed, throwing on null), and `System.exit`. An
  `if` whose condition is a constant expression (`if (ENABLE_BLINDING)` of a `static final
  boolean`) keeps only the branch that runs, since Java's rules for which locals are assigned
  take the condition as known and Rust's do not.
- Integer overflow wraps as in Java; division by zero throws `ArithmeticException`; float
  arithmetic and printing follow Java's rules (`Double.toString` and `Float.toString` exactly).
- Annotations and annotation types are left out, since they do nothing at run time. The class
  literal of an annotation type is a `Class` that `isAnnotation()`, named by its binary name
  (`Outer$Skip` for one declared in a class; not one declared in a local or anonymous class), and a value of an
  annotation type is an `Annotation` where the program has that interface; a class implementing
  one, and any other use of one, is rejected. `isAnnotationPresent(A.class)` is answered from the
  declarations (`lower/annotation_presence.rs`), and so are `getAnnotation(A.class)` of a class
  and the annotations of reflection's methods, fields and constructors, annotations whose members
  give the declared values and defaults (`lower/annotation_values.rs`). Type annotations (`java.lang.@Nullable String`,
  `@Flags int`) are dropped from the types javac prints (`typeText` in `java/JavaTree.java`),
  also from the wildcard of a captured type (`capture#1 of ? extends java.lang.@Nullable Object`,
  a value read through `Stream<? extends @Nullable Object>`), which javac's `stripMetadata` keeps.

## Strings, builders, characters and formatting

- **Strings** are WTF-8 (`java::JString`, see object-model.md, 4): a string holds Java's code
  units exactly, lone surrogates included, and halves of a pair that a concatenation or an append
  brings together join. The `String` methods count in UTF-16 units as Java does (`indexOf`,
  `substring`, `charAt`, the code point methods) and throw what Java throws, with Java's
  messages.
- **`StringBuilder` and `StringBuffer`** are `java::StringBuilder`, a shared handle to UTF-16
  units: every holder sees changes, and `append` chains stay chains. Where two threads may reach
  builders they are `java::SharedStringBuilder`. Their capacity grows as Java's does (twice
  plus two, from the byte length when text beyond Latin-1 first arrives, with room for the
  longest `double` or `float` text before appending one), so `capacity()` and `trimToSize()`
  answer what Java's do. `append` and `insert` take every overload, ranges of a `char[]` or a
  `CharSequence` among them, and the bounds checks throw Java 26's exceptions with its
  messages; inserting a range of a `CharSequence` that a program class implements is rejected
  (Java reads it with `charAt` after making room for it).
- **Characters**: the `Character` tests and conversions, with their `int` code point overloads,
  use tables generated from the JDK's data. Case conversion uses Rust's Unicode tables with Java's
  exceptions, and `isAlphabetic` uses Rust's `is_alphabetic`; both may differ from Java on code
  points the corpus does not test. In a whole program the rest of `Character` (`getType`,
  `UnicodeScript`, `UnicodeBlock`, ...) is the JDK's, with its `CharacterData` tables.
  `toUpperCase`/`toLowerCase` use the root locale; with a `Locale` (whole program) they do too,
  except Turkish, Azeri and Lithuanian, which end the program.
- **Numbers**: the common `Math`, `StrictMath`, `Integer`, `Long`, `Short`, `Byte`, `Double`,
  `Float`, `Boolean` and `Character` methods are hand-written in the runtime with Java's exact
  results: `floorDiv`/`floorMod`, the `*Exact` methods, `multiplyHigh`, `fma`, `ulp`, parsing
  with a radix or over a range, `decode`, the unsigned methods, `toHexString` of doubles
  (`hex_float.rs`), and `parseDouble`/`valueOf` with Java's grammar and rounding. Their
  `NumberFormatException` messages are Java's. `StrictMath` is a port of fdlibm and matches Java
  bit for bit; so does `Math` where HotSpot uses fdlibm. HotSpot's own `Math.sin`, `cos`, `tan`,
  `exp`, `log`, `pow`, `cbrt`, `sinh` and `tanh` cannot be ported and use the platform's libm,
  which agrees on all but about 0.1 percent of arguments.
- **Regular expressions** (`split`, `replaceAll`, `replaceFirst`, `matches`) run a small
  backtracking engine in the runtime (`regex.rs`): literals, escapes, classes, `\R`, groups
  (atomic too), lookahead, back references, alternation, anchors, the word boundaries `\b` and
  `\B` (Java's: an ASCII word character, or a non-spacing mark after a letter or digit), and
  greedy, lazy and possessive quantifiers. Lookbehind, flags, named groups, `\p{..}` and set
  intersections are rejected in a constant pattern and throw `UnsupportedOperationException` in
  a computed one. In a program translated with the JDK's sources, such a constant pattern and
  every computed one are the JDK's own `Pattern`'s instead, as Java's `String` calls it
  (`lower/regex.rs`).
- **`String.format`** is Java's `Formatter` in the US locale for `%b %h %s %c %d %o %x %e %f %g
  %n %%` with indexes, flags, width and precision, rounding half up from the shortest decimal as
  Java does. Arguments keep their Java type (`java::Arg::Long(n)`), so a mismatch throws
  `IllegalFormatConversionException`. `%t` and `%a` are rejected in a constant format, and so
  is an array argument other than the `Object[]` of all of them. With a `Locale`,
  `String.format` and `printf` are the JDK's `Formatter` in that locale (a whole program; without
  the JDK's sources they are rejected).
- **Charsets**: `getBytes` and `new String(bytes, ..)` for UTF-8, US-ASCII, ISO-8859-1, UTF-16,
  UTF-16BE, UTF-16LE, UTF-32, UTF-32BE and UTF-32LE (`charset.rs`, where UTF-32 keeps a
  surrogate it decodes, as Java does), and for the JDK's single-byte charsets (windows-1252
  and the other Windows code pages, the ISO-8859 ones, KOI8-R, the IBM and Mac code pages: 103
  on JDK 26). Those are tables of the JDK that the translator was built with, taken from its own
  coders (`java/Charsets.java`, `jdk_charsets.rs`), and the translator asks the JDK it runs
  with again where that is another one. A program that looks a charset up by name carries the
  tables it may need: the ones its constant names name, or all of them where it computes a
  name. A `Charset` is an ordinary value, which can also be held as an `Object`; `forName` resolves aliases and case as Java does, and
  gives the same value for each name of a charset, and `defaultCharset()` is UTF-8 (JEP 400).
  An unknown or malformed name throws `UnsupportedCharsetException` or
  `IllegalCharsetNameException` with Java's message, which a `catch` sees. A charset that Java
  has but that is none of these (Shift_JIS, GBK and the other multi-byte ones, and
  x-IBM1129, which encodes a second char to its replacement byte) is rejected where a constant
  names it; where the program computes its name, it carries the names of those charsets too,
  and `forName`, `isSupported`, `getBytes` and `new String` throw
  `UnsupportedOperationException` for one, which the census lists. `Charset.aliases()` and a
  `Charset` array element that starts out null are rejected. In a whole program
  `newDecoder()`/`newEncoder()` give the JDK's own coders of the nine charsets and, made from
  the tables, the JDK's `SingleByte` coders of the others
  (`java/overlay/sun/nio/cs/TranslatedCoders.java`).
- `==` on strings compares what Java compares: by text where both sides can only be interned
  strings, as null against null where no string can be on both sides, and otherwise by the
  objects, with every string of the program shared as Java shares it (literals interned, new
  strings new, the JDK methods that return their receiver returning it). A comparison of a
  string from a library call that the translation does not follow is rejected, naming the call.
  See [object-model.md](object-model.md), section 1.6.

## Classes and objects

- Static and instance methods, overloading, varargs, fields with initializers, initializer
  blocks, static fields, constructors (with `this(...)` and `super(...)`), and a user `toString`
  in printing and concatenation. A `toString` is a method returning the string.
- Constructors build a plain value, starting from a struct literal of the leading field
  assignments (`P { x, y }`), and `new` shares it (`Arc::new(RefCell::new(P::new(1, 2)))`).
- An object parameter that a method only uses (fields, calls, comparison, printing, a copy into a
  local) is a borrowed `&Arc<..>`, or `&Option<Arc<..>>` where it may be null. Methods of shared classes borrow their object
  (`p.borrow_mut().set(2)`) when nothing they run can reach the object another way.
- `==` on objects is identity: `Arc::ptr_eq`, or `java::same` where either side may be null. Two
  `Object`s that may both be strings or boxes are compared at run time, and equal ones end the
  program, as the translation does not keep their identity.
- `super.clone()` of `Object` is a shallow copy of the runtime class, a `match` on the variant in
  a class tree; a class that is not `Cloneable` throws `CloneNotSupportedException`. `clone` of
  an object other than `this` is rejected. `super.finalize()` of `Object` does nothing, as
  `Object`'s `finalize` is empty.
- A covariant override's value is converted to the overridden method's representation.

## Class trees, interfaces, `Object` and generics

Design: [object-model.md](object-model.md).

- **Class trees**: a class with subclasses keeps its fields in a struct (`ShapeFields`) that
  each subclass embeds as `base`. A type that objects of several classes can be is an eight-byte
  reference holding the class's number and the object's handle, read through a Rust `enum` with
  a variant per class (`ShapeView::Circle(&Arc<RefCell<Circle>>)`); a virtual call is a method of
  the type that matches the class (`codegen/dispatch.rs`). Where every value of a type is one
  concrete class, the value is that class's object and an upcast is free.
- A constructor that uses `this` as a value or calls an overridable method switches its tree to
  Java's construction order (`analysis/whole_objects.rs`): the object is allocated zeroed and each
  class's constructor body runs on it, superclass first.
- **Interfaces**: abstract, default, private and static methods, constants, interface
  inheritance, `Iface.super.m()`, and enums implementing interfaces. An interface type is an enum
  over its implementing classes, or that class's object when there is only one.
- **`instanceof` and casts** are `matches!`/`if let` on the type's enum, including patterns,
  record patterns, negated patterns (`if (!(x instanceof T t)) return;` becomes `let .. else`,
  and the branches of `!(o instanceof T t) ? false : ..t..` swap), `||` chains with a negated
  pattern, and an `if` whose negated `&&` chain or `||` inside an `&&` chain binds a pattern
  (`lower/negated_patterns.rs`). A failed downcast throws `ClassCastException`.
- **`Object`** is an enum with a variant per class of the program whose values become `Object`s,
  plus strings, boxes, arrays, builders and class values (`Object::String(..)`,
  `Object::Integer(5)`, `Object::IntArray(..)`). `equals`/`hashCode` are the program's where
  defined, otherwise identity for objects and value for strings and boxes. Default `toString`
  prints the binary name and hash code (`Point@1b6d3586`). `new Object()` is `PlainObject`.
  `==` between two `Object`s that may both be strings or boxes compares them at run time
  (above), except in `a == b || a.equals(b)`, where `equals` decides either way; a points-to
  analysis (`analysis/object_values.rs`) decides which sides may be. An atomic compare of
  `Object` elements that may be strings or boxes is rejected. Where the program's strings are
  shared (above), strings in them compare as objects.
- `Comparable`, `Number`, `CharSequence` and `java.lang.Enum` used as types are enums like
  `Object` over the program's implementors and the matching library values.
- **Generics** translate erased, as javac compiles them, with javac's casts and bridge methods.
  A generic class or method that only stores, returns and passes its type variables is a Rust
  generic instead (`struct Stack<T>`, `fn choose<T: Clone>`), with no casts. A type variable
  that javac's inference makes up, for a parameter bounded by itself with nothing to infer it
  from (`<B extends Builder<B>> B newBuilder()`), is its erasure.
- An array of references used as an `Object[]` is an `Object[]` throughout. Where the code makes
  one, it carries the class it was made of, so storing an object of another class throws
  `ArrayStoreException`, `getClass()` is its own, and `instanceof` and casts to an array class
  test it; an array of arrays and `new Object[n]` carry none and pass those
  tests, as does an array that `Array.newInstance` makes of a class the code neither makes nor
  tests arrays of.

## Enums and records

- An **enum** is a `Copy` Rust `enum`. Fields with constant values per constant are accessors
  that `match` on `self`; an enum whose constructor does more (calls, `this(..)`, null or computed
  arguments) keeps its fields in tables indexed by ordinal, filled by running its constructors in
  its static initialization, as Java does. Constant bodies override methods as a
  `match self`, also an interface's method and `toString()` that the enum itself does not
  declare: the constants without an override run the interface's default method, or answer
  their names. `valueOf` of an unknown name throws `IllegalArgumentException` with Java's
  message. An enum value that may be null is an `Option` of the enum. Using a constant
  initializes the enum first, as in Java.
  A body's methods that override nothing are methods of the enum. A body's instance field of an
  object type gives the enum the table form, and its static fields are the enum's
  (`javac/enum_bodies.rs`), initialized after every constant rather than as its constant is
  built. Rejected in a constant's body: blocks and member classes, and fields whose name or type
  clashes with the enum's or, for a static field, another body's.
- A **record** is a class with the members Java gives it (written into javac's tree by
  `javac/records.rs`): canonical or compact constructor, accessors, `toString`, `equals` and
  `hashCode`. A `switch` with patterns (`case Circle c when ..`, `case byte[] b`, `case null`,
  record patterns, nested records) is an `if let` chain, exhaustive over a sealed hierarchy or
  enum without `default`. The components of a generic record's pattern have the erasures its
  fields hold; rejected: a record pattern of a record that is a Rust generic. An accessor written
  in the record runs at each use of a pattern variable, not once. `java.lang.Record` is a type like `Object`: a
  value of it holds any of the program's records.
- Local records, enums and interfaces declared in a method or an initializer block move to the
  top level.

## Nested classes, lambdas and closures

Design: [closures-and-nested-classes.md](closures-and-nested-classes.md).

- **Inner, local and anonymous classes** are lowered as javac desugars them (`lower/captures.rs`,
  `lower/inner.rs`): an inner object holds its enclosing object in the field `outer`, and the
  locals a local class reads become fields set from its constructor. `getName` gives javac's
  binary names (`Main$1Labeler`, `Main$1`). An inner class tree takes Java's construction order
  as any other (above), passing each class's enclosing instance and captured locals on, upcast
  where a superclass is an inner class of the enclosing class's superclass. Not yet: such a tree
  where a constructor passes them on other than as they are (`analysis/whole_objects.rs`).
- **Lambdas and method references** (`lower/lambdas.rs`) are each a class implementing the
  functional interface, with the captured locals as fields. A bound method reference evaluates
  its receiver once and checks it for null, as Java does.
- **Closure types** (`analysis/closures.rs`): a functional interface whose values are only
  lambdas and method references that nothing treats as objects is a type alias for
  `Rc<dyn Fn(..) -> ..>`, and each lambda is a Rust closure. Its default methods are an extension
  trait. Not yet: typed closures per instantiation of a generic interface, and `impl FnMut`
  parameters.
- **For-each over an `Iterable`** (`lower/foreach.rs`) runs the program's or the JDK's
  `iterator`, `hasNext` and `next`, so `ConcurrentModificationException` happens where Java's
  does.
- In a single file the JDK's functional interfaces have their abstract methods only
  (`java/Functional.java`); calling one of their default or static methods is rejected with a
  pointer to whole-program mode. `super` in a lambda other than as a method call's receiver is
  rejected.

## Arrays and `java.util.Arrays`

- Arrays of every type and of any depth. The faithful form is `java::Array<T>`; arrays that no
  other name can see are `Vec<T>` or `Vec<Vec<T>>` (`analysis/arrays.rs`), converted once with
  `java::share` where they meet code that needs a shared one. Parameters become `&[T]` or
  `&mut [T]` where every call can lend one.
- Indexing is `a[i as usize]` where the bounds analysis proves the index in range
  (`analysis/arrays/bounds.rs`), and `java::at`/`java::put` otherwise. An innermost counting loop
  can check its whole range once and then run over slices without checks
  (`analysis/arrays/slices.rs`).
- `java.util.Arrays` on primitives and strings is hand-written (`equals`, `hashCode`, `sort`,
  `fill`, `copyOf`, `copyOfRange`, `binarySearch`, `deepToString`) with Java's results.
  In a whole program every other `Arrays` call runs the JDK's translated code. Without the JDK's
  sources, `asList`, `stream` and `equals(null, b)` are rejected.
- `System.arraycopy` between arrays of the same element type, and between arrays held as
  `Object`s of any two types, with HotSpot's checks of each element and its
  `ArrayStoreException` messages. Between typed arrays of two classes (including `Object[]`) it
  copies them with HotSpot's checks too; between a typed `String[]` and a class's array, and
  between typed arrays of arrays, it is rejected.

## Exceptions

Design: [exceptions.md](exceptions.md).

Exceptions are values. A method returns `Result<T, Throwable>` only when some caller catches
what it may throw, and `?` passes it on; every other method keeps its plain signature, and an
exception that nothing catches ends the program where it is thrown, printing
`Exception in thread "main" ...` and exiting with code 1. `try` is a labelled block evaluating to
a `Result`, and its catch clauses are a `match`. Supported: the program's own exception classes,
the standard ones (`crates/translator/java/Standard.java`, given to every program), multi-catch,
`finally` on every exit path, try-with-resources with suppressed exceptions, causes, failures
of the runtime (division by zero, index out of bounds, null dereferences, failed casts) as the
exceptions Java throws with Java's messages, and static initializers that throw
(`ExceptionInInitializerError`, then `NoClassDefFoundError`). With `--keep-going`, code that
names a class missing from the roots throws `NoClassDefFoundError` where it runs, as on the JVM
([exceptions.md](exceptions.md#classes-missing-from-the-roots)). Stack traces are empty, as the
specification allows ([exceptions.md, "Stack traces"](exceptions.md#stack-traces)).

What is still rejected is listed under "Not done" in [exceptions.md](exceptions.md#not-done).

## Threads

Design: [multi-threading.md](multi-threading.md).

Threads are real platform threads. `Thread` (subclassed or given a `Runnable`), `start`, `join`,
`sleep`, names, ids, priorities, daemon threads, interrupts, uncaught-exception handlers and
thread groups behave as Java's; `java.lang.Thread` is `crates/translator/java/Threads.java` over
the runtime's natives (`thread.rs`). In a program that can start a thread, the objects threads
may share are `Arc<C>` of `java::SharedField`s (`Arc<java::Monitored<C>>` where a monitor site may
lock them, `Arc<C>` of plain fields when immutable), arrays that threads may reach are
`java::SharedArray`, or `java::Frozen` where only a constructor or a class's initialization writes
them and the program then only indexes and measures them,
and the statics that a started thread can touch are process-wide; a program without threads
translates exactly as before.
`synchronized`, `wait`, `notify`, `notifyAll` and `holdsLock` are Java's, kept only where the
program can tell. In a whole program `java.util.concurrent` (atomics, `ConcurrentHashMap`,
`ForkJoinPool`, `LockSupport`) and `ThreadLocal` are the JDK's translated code.
`wait(millis, nanos)` waits a millisecond longer for any nanoseconds, as `Object`'s does.

An array's monitor is kept too, made the first time a thread takes it
([multi-threading.md](multi-threading.md), 3.1). Rejected: the monitor (`synchronized`, `wait`,
`notify`) of a string, a box or another value that has no identity in the translation
(`lower/sync.rs`).

## Class values and reflection

Design: [reflection-and-plugins.md](reflection-and-plugins.md).

- Comparing the classes of the program's objects (`getClass() == o.getClass()`, `x.getClass() ==
  Dog.class`) is a `matches!` of the variant. Any other class is a `&'static java::Class`, a
  reference to the class's static or constant, one word wherever it is held, and equal by binary
  name (`equals` of an object that is not a class is false), with `getName`, `getSimpleName`, `getTypeName`, `getSuperclass`,
  `isArray`, `isInstance`, `cast`, `isAssignableFrom` and their relatives. Each class value
  carries the binary names of the classes and interfaces above it, which `isAssignableFrom` tests;
  `isInstance` and `cast` test the names above the value's class, each name compared by its
  length and last eight bytes before it is compared whole (`java::Class::is_instance`). A class
  that nothing below it can have instances of (a final class of the runtime, or a class or enum
  of the program that no class of the program extends, other than a `Throwable`, which the
  runtime makes by name) is marked a leaf, and they test only the first name, the value's own
  class. A leaf of the program also carries its class number (`Class::numbered`): where the
  translation has the value's class number (a value of an enum of classes carries it, whether a
  variable, an array's element, a field or a call gives it, which is then evaluated once, after
  the class; a variable of a leaf's type has the leaf's), `isInstance` and `cast` compare the
  numbers and ask the value for its names only when they differ, to throw
  (`is_instance_numbered`).
  `getModifiers`, `isMemberClass`, `isLocalClass`, `isAnonymousClass`, `getEnclosingClass` and
  `getDeclaringClass` answer from what the class value carries in a program that asks for them:
  the modifiers javac gives the class, implicit ones included, and where it is declared
  (`TypeDecl::modifiers` and `enclosing`). An enum constant with a body is of its body's class,
  an anonymous subclass of the enum named as javac names it (`Op$1`, `EnumConstant::body`), and
  `getDeclaringClass()` of any constant is the enum. A record's superclass is `java.lang.Record`,
  and `Object` has none. `getInterfaces()` of a class that is no class literal is a new array
  filled from the interfaces that the class value carries in such a program, in the order the
  source names them (`TypeDecl::interfaces`), the runtime's classes with the JDK's lists and an
  array with `Cloneable` and `Serializable`; an interface that the translation has no type of
  (`Serializable`, `Cloneable`, a marker) is a class value of its name, which extends nothing.
  `getSigners()` is `null`, as for a class without signers: a translated class comes from no jar,
  even where Java's came from a signed one (the Minecraft server's `ModCheck` then reports the
  server as very likely modified). A class held as an `Object` is `Object`'s `Class` variant, and one held as a
  `java.lang.reflect.Type` is `Type`'s.
- `getGenericSuperclass()` is a chain of tests of the class against those whose superclass has
  type arguments (written at the call when the call narrows the class down, as `getClass()` and
  a class literal do; otherwise one chain over the program's classes, shared by every call),
  each making the type javac gives as the JDK's own `ParameterizedTypeImpl`,
  `WildcardTypeImpl` and `GenericArrayTypeImpl` (`lower/generic_types.rs`), so a type token
  (`new TypeToken<List<String>>() {}`) reads its type argument as in Java.
  `getGenericInterfaces()` and `getTypeParameters()` are made the same way, and a class's type
  variables are the JDK's `TypeVariableImpl`, with their bounds and the class that declares them,
  so gson's walk from a field's `List<String>` up to `Collection<E>` resolves `E` to `String`. A
  method's type variable is not made: a type that mentions one throws
  `UnsupportedOperationException`. The generic types of methods are still rejected.
- `Class.forName` of a constant name is that class, and of an array's descriptor
  (`"[La.B;"`) that array's class; of a computed name, a `switch` over the classes code can find
  by name. An unknown name throws `ClassNotFoundException`.
- **Handles**: `Unsafe` offsets, field updaters and `VarHandle`s of fields and array elements,
  and the `MethodHandle`s of the handle factories, are the accesses they name, and a
  `MethodHandle` of a static method (`findStatic`) is a call of it, followed through
  constants, locals, fields and calls ([multi-threading.md](multi-threading.md), 5.3). A call of
  a lookup's, a `MethodHandle`'s or a `MethodType`'s method that the translation does not follow
  (a handle of any other method, `invokeWithArguments`, combinators such as `filterArguments`,
  `MethodType.insertParameterTypes`) throws
  `UnsupportedOperationException` where it runs, as a JDK that lacks the method would fail, and
  code that falls back on a failed lookup, as netty does, takes its fallback; a census lists each
  such call. A handle used as a value any other way is rejected.
- Reflection over the program's classes (`getMethod`, `getField`, `Method.invoke`,
  `Constructor.newInstance`, ...) goes through a table of members built into the translation for
  the names that reachable lookups can find (`lower/reflection.rs`), and in a whole program for
  the classes whose `Class` values those lookups can get, from a class literal or not
  (`analysis/reflection_table.rs`). `ServiceLoader` finds the providers that `module-info.java`
  and `META-INF/services` declare.
- A static method that asks for its caller's class (`StackWalker.getCallerClass()`,
  `Reflection.getCallerClass()`) gets it as an extra parameter from each call
  (`lower/callers.rs`). Asking anywhere else, and `StackWalker.walk`, are rejected; a walker
  itself is made with any of `getInstance`'s options. A security manager's `getClassContext()`,
  the classes on the call stack, throws `UnsupportedOperationException`, which log4j and slf4j
  catch to fall back; a census lists it.

## Output, input and the native layer

Design: [natives-and-input.md](natives-and-input.md).

- `System.out` and `System.err` printing is buffered with Java's 128-byte buffers
  (`java::println!`); `System.err` flushes `System.out` first so the two stay in order.
- With the JDK's sources, `System.out`, `System.err` and `System.in` can be values, wrapped in
  the JDK's streams, readers and writers or the program's own subclasses, and `System.setOut`
  and `setErr` replace them: the program's calls on `System.out` then run the new stream's
  methods. Without them, using one as a value is rejected. `PrintStream`, `InputStreamReader`
  and `OutputStreamWriter` are stand-ins that use the runtime's charsets.
- Native methods of translated JDK classes are hand-written in the runtime and chosen by class,
  name and signature (`ast/natives.rs`); any other native is rejected with its name. This covers
  the clocks, `identityHashCode`, the bit conversions of floats, file descriptors and files
  (`fd.rs`, `fs.rs`), `java.nio.file` over a patched `UnixNativeDispatcher` (`unix.rs`), name
  lookups for `InetAddress` (`net.rs`), `Unsafe` memory outside the heap (`native_memory.rs`),
  and the `jrt:` file system over the JDK's `lib/modules` image.
- `System.getProperty` gives what a plain `java Main` run gives (`lower/system.rs`): machine
  values at run time, other keys fixed from the JDK the translator ran with, and
  `java.awt.headless` `"true"`, as with `-Djava.awt.headless=true`. A key that code computes is
  a `switch` over the keys of such a run; `Integer.getInteger` and `Long.getLong` of one give
  the number of a key whose value is one, and the default for any other key.
  `sun.java.command` and `user.timezone` as constants are rejected. `System.setProperty` and
  `clearProperty` keep their changes, which every `getProperty` of a program that calls them
  reads (`java/overlay/java/lang/TranslatedProperties.java`). `System.getenv(name)` reads the
  environment, and in a whole program `System.getenv()` is the JDK's map of it.
- `Runtime` is no value: its methods are translated on `Runtime.getRuntime()` or a local that
  holds it, and any other use of one is rejected. Its shutdown hooks run on the first exit, as
  Java's do, a `SIGTERM`, `SIGINT` or `SIGHUP` too ([multi-threading.md](multi-threading.md), 6).
- A translation is headless, unlike a JVM on a machine with a display: AWT's natives are not
  translated, so `GraphicsEnvironment.isHeadless()` is always true, and a program that would
  open a window there (the Minecraft server without `nogui`) runs as it does on a server
  without one. The tests run Java with `-Djava.awt.headless=true` to compare.
- There is no security manager (`System.getSecurityManager()` is null), though a class of the
  program may extend `SecurityManager` and make its objects, as log4j's does; no virtual threads; and
  sockets only of TCP: `java.net`'s and `java.nio.channels`' TCP sockets and selectors run on the
  system's ([natives-and-input.md](natives-and-input.md)), but datagram, Unix domain
  and SCTP sockets are not translated. The collector runs on its own
  ([collector.md](collector.md)): `gc()` does nothing, and `Runtime`'s memory figures are fixed
  values.

## Whole programs

- javac compiles all sources in one run, and a reachability pass (`program/reach.rs`) starts at
  `main` and follows every resolved call, `new`, field access and type, dispatching calls to the
  overriders in every instantiated class. Only what `main` can reach is lowered. Library roots
  are only on javac's source path, so javac attributes a library file only when reachability
  asks for one of its classes.
- Reachability stops at the classes the runtime provides (`RUNTIME_CLASSES`: `String`,
  `StringBuilder`, `System`, `Runtime`, `Math`, the boxes, `Object`, `Class`, `Thread`, the
  standard charsets) and at what is not translated (`UNTRANSLATED`: most reflection, method
  handles but those above, serialization, class loading, and security but its exceptions, the
  lookup of services by name and the services translated so far, see
  [natives-and-input.md](natives-and-input.md#class-loading-resources-and-classes-by-name)).
  An untranslated class used as a type has no objects, so its variables are only ever null,
  and reflection finds no member of its class literal (`program/reach/stubs.rs`). Code that only JVM flags enable (Java Flight Recorder,
  `Tripwire.ENABLED`, virtual threads) is not reached, and neither are parallel streams when no
  stream can become parallel (`program/reach/sequential.rs`).
- A source root may be a module with `module-info.java`; the whole translation is one closed
  program, and `Class.getModule()`, `Module` and `ModuleLayer.boot()` answer from the declared
  modules (`program/modules.rs`). Defining a new module layer throws.
- Each package becomes a Rust module (`com.example.util` is `com::example::util`; the JDK's
  `java.util` is `java_::util`). `--crate DIR` writes a file per package, `-o FILE` one file, and
  `--crate DIR --crates N` a workspace of a header crate and `N` body crates that build in
  parallel ([several-crates.md](several-crates.md)).
- `--census` translates past every rejection and reports what is rejected and why, grouped by
  message, separately for program and library code (`program/census.rs`). A rejected method
  becomes a stub that ends the program with the rejection if it runs; a statement that lowering
  rejects in a method that is no constructor, or in an initializer block, is stubbed with the
  rest of its block instead (up to eight a method or block), so code rejected for a statement
  behind a test that is false keeps its other statements (`lower/census.rs`). Either way the code
  counts as rejected. It lists apart, as code that translates, the code that throws
  `NoClassDefFoundError` for a class missing from the roots (`--keep-going`), the handle calls
  that throw `UnsupportedOperationException` because the translation does not follow them, and
  the charset names known only at run time, which throw it for a charset of the JDK that the
  translation does not have. `--survey` is a quicker syntactic view of what the reachable code needs
  from outside the translated sources (`program/survey.rs`). Both have `-json` forms.
  `scripts/jdk-coverage.py` runs the census over the target programs and writes
  [jdk-coverage.md](jdk-coverage.md).
- `--modules-json FILE` writes the module tree and what each item names, the dependency graph
  that shows which groups of classes could be split off ([several-crates.md](several-crates.md)).

## Idiomatic output

Plainer forms are chosen only where the meaning is the same. A counting loop
`for (int i = a; i < b; i++)` becomes `for i in a..b` when nothing can change the bound or the
counter while it runs. A final `return x;` becomes the tail expression, and `T x = e; return x;`
becomes `e`. Parentheses appear only where precedence needs them, compound assignments use
`op=` where they do what Java's do, and a range proof lets `+ - *` replace `wrapping_*`. A
null check narrows the value (`if let Some(s) = s.as_deref()`, `let .. else` after an early
return). `scripts/clippy-corpus.sh` counts the clippy and rustc warnings in the generated code
of the corpus. [natural-rust.md](natural-rust.md) has the plan for output that reads like
hand-written Rust.

## Not done

Beyond the rejections listed in each section above:

- In a single file, every library class the runtime does not supply (collections, streams,
  `Scanner`, `java.io` beyond the standard streams). Translate the program whole instead.
- Objects held in one place as owned fields, `Box`es or `Vec` elements: see
  [thorough-mode.md](thorough-mode.md) and [natural-rust.md](natural-rust.md). An object that
  never leaves its method is already made on its stack
  ([collector.md](collector.md#objects-on-the-stack)).
- `ArrayList` as a `Vec` and sequential streams as iterators: [lists-and-streams.md](lists-and-streams.md).
- Not measured yet: the share of values that became owned or plain, the remaining `Option`s and
  null checks, and dynamic calls. The time and peak memory of the benchmarks are in
  [native-speed.md](native-speed.md).
