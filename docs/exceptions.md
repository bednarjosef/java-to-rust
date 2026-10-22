# Exceptions: `throw`, `try`/`catch`/`finally`, the standard exception classes

Exceptions are values: a method that can throw something its callers catch returns
`Result<T, Throwable>`, `?` passes the exception on, and every other method keeps its plain
signature. This document describes how that works, why, and what is still rejected. It builds on
the object model (`docs/object-model.md`): exception classes are ordinary class trees there.

## The decisions in brief

1. **Exception classes are classes of the object model.** A program's own exception classes are
   ordinary classes. The standard ones (`Throwable`, `Exception`, `IllegalArgumentException`, ...)
   are classes the translator gives to the program, so fields structs, enums, dispatch, `Display`
   and identity work for exceptions unchanged, and an override of `getMessage` or `toString` is
   dispatched like any override.
2. **The standard classes are the translator's own Java source**, not the JDK's
   (`crates/translator/java/Standard.java`). Reachability never reads the JDK's `Throwable.java`
   (stack walking, serialization, natives), and a single file and a whole program translate them
   the same way.
3. **`Throwable` is an enum** with one variant per exception class whose objects exist (created
   by the program, or raised by the runtime where the program can see it). It stays an enum even
   with one variant, so the runtime's failures convert into it (`impl From<java::Exception> for
   Throwable`, which `?` uses). Standard types between `Throwable` and the concrete classes
   (`Exception`, `RuntimeException`, ...) declare no members of their own, so they are represented
   by `Throwable` too: no conversions between them are needed, and `catch (RuntimeException e)`
   binds the matched value as it is.
4. **Which methods throw is an analysis** (`analysis/exceptions.rs`). A method returns `Result`
   exactly when some caller observes an exception that may escape it. Everything else ends the
   program where it is thrown, which is what Java does once nothing catches it.
5. **Failures of the runtime become values only where they are observed.** Division by zero,
   bad indexes, null dereferences, malformed numbers and the like normally print and exit
   (`java::throw`). Where a `catch` or a `finally` could see one, the operation uses a checked
   form that returns `Result<_, java::Exception>` instead. The fast exiting form stays the default.
6. **`try` is a labeled block** that evaluates to a `Result` (or a `java::Completion` when a
   `return` or a jump has to pass through a `finally`); the `catch` clauses are a `match`.
7. **Uncaught exceptions** end the program as Java's do: `Exception in thread "main" ` and the
   exception's stack trace on standard error, exit code 1. Stack traces have no frames (see
   "Stack traces").
8. **Static initializers return `Result` only where a `catch` observes them.** The uncaught case
   prints `ExceptionInInitializerError` and exits without any `Result` in the code.

## The standard classes

`Standard.java` is written in package `standard` in the subset of Java the translator supports.
`build.rs` prints its syntax tree with `JavaTree` at build time; lowering (`lower/exceptions.rs`)
gives each class its real package (`java.lang`, or the one the table `OTHER_PACKAGES` names) and declares the classes the program names, with their superclasses, and of
those only the constructors and methods the program uses (with what they use in turn). A
standard class is abstract unless the program creates its objects or the runtime may raise it
where it is seen, so `Throwable`'s enum only has the variants it needs.
`program/reach.rs` (`provided`) stops reachability at these classes. An exception class of the
JDK that `Standard.java` does not have is translated from the JDK's source in a whole program,
and rejected in a single file.

The file holds `Throwable`, `Exception`, `Error` and their common subclasses in `java.lang`,
`java.util` and `java.io` (including the `IllegalFormatException` family that `String.format`
raises, `ExceptionInInitializerError` and `NoClassDefFoundError`), and `StackTraceElement`; the
file itself is the list.

`Throwable` keeps a message, a cause (with a flag for whether `initCause` set it, since Java's
field holds the object itself to mean "not set yet"), suppressed exceptions and frames.
Constructors with `enableSuppression` / `writableStackTrace` set to false are honoured.
`toString` is `<class name>: <message>` or the class name alone, where the class name is
`Builtin::ClassName`: Java's binary name of the object's class (`Bank$InsufficientFunds`).
`getClass().getSimpleName()` is the same builtin with `simple: true`.

`Standard.java` cannot `throw` (javac sees its `Throwable` as unrelated to
`java.lang.Throwable`), so `lower/suppress.rs` adds Java's checks at the start of the lowered
methods: `addSuppressed` of itself (`IllegalArgumentException("Self-suppression not
permitted")`) and of `null` (`NullPointerException("Cannot suppress a null exception.")`),
`initCause` set twice (`IllegalStateException`) or to itself (`Self-causation not permitted`),
and the null checks of `setStackTrace` and of `StackTraceElement`'s constructor.

Classes that pass their constructor parameters to `super(...)` keep their own fields structs
(`IllegalArgumentException { base: RuntimeExceptionFields }`), the object model's ordinary
layout; it costs nothing at run time. A synthetic class's Rust name is its simple name; a
program class named like a standard class the program uses is rejected.

## Values of exception types

| Java type of the value | Rust |
| --- | --- |
| a class with one concrete class under it | that class's object, `Rc<C>` (or `Rc<RefCell<C>>` when the family is mutable) |
| `Throwable`, and a standard class with several concrete classes under it | `Throwable` |
| a program class with several concrete classes under it | the enum of that class, as for any class tree |

`throw e` and every `Err` convert the value to `Throwable` (wrapping an object in its variant, or
`to_throwable()` from a program class's enum). A `catch (T e)` is a `match` arm over the variants
of the classes under `T`. It binds the variant's object when `T` has one concrete class, the
matched value itself when `T` is represented by `Throwable`, and otherwise the value converted to
`T`'s enum with an inline `match` of the arm's variants. Multi-catch is one arm whose pattern is
the union. A catch type that nothing the body throws falls under is not written. Arms keep the
order of the clauses, so the first matching clause runs, as in Java.

An exception object is never owned: it is converted to `Throwable` when thrown. Exception classes
are immutable unless the program writes their fields after construction.

## Methods that throw

In a method that returns `Result`, `return v` is `return Ok(v)`, a body that completes normally
ends with `Ok(())`, `throw e` is `return Err(..)`, a call of a throwing method is `call?`, and an
observed runtime failure is a `java::try_` function of the runtime with `?` (`java::try_non_null(..)?`, `java::try_floor_div(..)?`). In a method that does not return `Result`, a
call of one that does (because other callers observe it) is `call.unwrap_or_else(java::uncaught)`,
and `throw e` where nothing catches it is `java::throw_uncaught(e)`: nothing here observes the
exception, so ending the program at once is what propagating it would do.

```java
static int parse(String s) throws Bad { if (s.isEmpty()) throw new Bad("empty"); return Integer.parseInt(s); }
```

```rust
fn parse(s: &str) -> Result<i32, Throwable> {
    if s.is_empty() {
        return Err(Throwable::Bad(Rc::new(Bad::new("empty"))));
    }
    Ok(java::try_parse_radix::<i32>(Some(s), 10)?)   // checked only if a caller observes it
}
```

- **Override groups agree**: if one member returns `Result`, all do, since a call may run any of
  them; the dispatcher forwards it unchanged.
- **Constructors** that throw something a caller catches return `Result<C, Throwable>`; `new`,
  `this(...)` and `super(...)` pass the failure on with `?`. A partly built object is dropped,
  which Java's program could not see either. An exception's own constructor may throw too:
  `throw new Fatal(d)` whose `d.toString()` fails throws that failure in place of the `Fatal`.
  The exception: the `(String)` constructors that the `From` impl builds the runtime's
  exceptions with must not throw, since that impl cannot fail; they run no program code.
- **Instance field initializers** run in each constructor that calls `super(...)`, outside every
  `try` of that constructor, so what they raise escapes the constructor
  (`analysis/exceptions/field_inits.rs`; the class's `field_inits` then returns a `Result`). A
  `this(...)` constructor runs none of them.
- **Rejected**: a constructor of an enum that throws something a caller catches (its constants
  are built where the enum is initialized, outside any `try`), and a call of `main` that is
  caught.

## `try`, `catch`, `finally`

A `try` is one of three forms, the plainest that is equivalent:

- **A plain block** followed by the `finally`'s statements, when nothing leaves it early: no
  `return` or jump out of it and no exception of the program (`Facts::raising_tries`).
- **The `Result` form** when only an exception can leave it early:

  ```rust
  let attempt: Result<(), Throwable> = 'attempt: {
      java::or_break!(from.borrow_mut().withdraw(amount), 'attempt);
      to.borrow_mut().balance += amount;
      Ok(())
  };
  let attempt = match attempt {
      Err(Throwable::InsufficientFunds(e)) => 'handler: { /* catch body */ Ok(()) }
      other => other,
  };
  java::println!("done");          // the finally
  attempt?;                        // or or_break!, or unwrap_or_else(java::uncaught)
  ```

  `java::or_break!(expr, 'label)` matches `expr` and breaks out of the labeled block with the
  error converted by `From`. Without a `finally`, the catch bodies are plain arms and the last arm
  passes on what no clause matches. A body that cannot complete normally is a block of
  `Result<Infallible, Throwable>`, its `return` is Rust's own, and the `match` has no `Ok` arm.
- **The `java::Completion` form** when a `return`, `break` or `continue` leaves the `try` and a
  `finally` must run first. `Completion<T, E, J>` has `Normal`, `Return(T)`, `Jump(J)` and
  `Throw(E)`; the jump is `break 'attempt Completion::Jump(n)`, and a `match` after the `finally`
  performs it. A block that no jump leaves has `Infallible` for `J`, one that no `return` leaves
  `Infallible` for `T`, and a program without exceptions `Infallible` for `E`, so the `match` has
  only the arms that can happen (exhaustive patterns over uninhabited variants, Rust 1.82). When
  neither body nor catch completes normally, the code after the `finally` diverges as Java's does.

Rules the generator keeps:

- A `return`, `throw` or jump inside the `finally` replaces how the `try` ended; the pending
  completion is dropped, as in Java.
- A `return` through a `finally` takes its value first; a `String` variable returned there is
  cloned, since the `finally` may read it. A `yield` or a constructor's `return` through a
  `finally` keeps the value in a slot outside the `try`, runs the `finally`, then yields or
  returns.
- A jump through several `finally`s goes to the innermost one, whose `match` makes the jump
  again for the next one out, so each runs in order. The jumps of one `try` are numbered in the
  order written.
- Rust forbids an unlabeled `break` or `continue` inside a labeled block (E0695), so a jump out
  of a `try` always names its loop's label; the `try` is a frame of `codegen/switch.rs` that is
  not a loop. Labels are written only when something breaks out of the block
  (`Gen::open_block`, `Gen::name_frame`), so none is unused.
- **Locals declared without a value and set in a `try`**: Rust's definite-initialization check
  does not follow the `Err` path, so such a local starts at a value Java never lets the code
  read, under `#[allow(unused_assignments)]`: Java's default for primitives and strings, an empty
  array, an enum's first constant, a plain box's zero. A local of an object type has no such
  value: a `try` without `finally` whose own statements set it carries it out in its `Ok` value
  (`Ok((a, b))`), which the `match` arm stores. Set anywhere else (only inside an `if` of the
  block, or in a `try` with a `finally`), it is rejected. A local declared inside the body itself
  is left alone, as Rust follows it there.

## try-with-resources

`lower/exceptions.rs` (`with_resource`) lowers it to `try` and `finally`, one nested `try` per
resource:

```java
{ R r = <init>; boolean r$failed = false;
  try {
      try { body } catch (Throwable r$thrown) {
          r$failed = true;
          try { r.close(); } catch (Throwable r$suppressed) { r$thrown.addSuppressed(r$suppressed); }
          throw r$thrown;
      }
  } finally { if (!r$failed) { r.close(); } } }
```

So resources close in reverse order, a resource whose creation fails closes the ones before it,
a `close` that throws after the body threw is added as suppressed, and the statement's own
`catch` clauses and `finally` go around all of it, as Java runs them after closing. An existing
variable (`try (r)`, `try (this.r)`) becomes `final var r$resource = r`, read when the statement
starts. A resource that is not a new object may be null, and a null one is not closed (the
`close` calls test it).

The resource's class must have a `close()` in the translated code ("a resource without a
`close()` of the program" otherwise). In a single file that means the program's own classes;
a library resource (`AutoCloseable` held as such, a lambda of it, the JDK's streams) is rejected
there.

## Failures of the runtime

The runtime raises a standard class with Java's message (`java::Exception { class, message }`).
Where no `catch` or `finally` could see the failure, the operation keeps its exiting form. Where
one could, `Gen::may_fail` picks the checked form, chosen by the same facts the analysis made:

- **A checked form of its own** (`Facts::raising`): `java::try_div`, `try_rem`, `try_get`,
  `try_put`, `try_modify`, `try_char_at`, `try_substring`, the `try_parse_*` family, `try_cast`
  for a downcast, the plain-array helpers (`try_at`, `try_row`, `try_row_mut`, `try_assign`, ...)
  and others in `crates/runtime`. An element update whose value may fail (`counts[i] += 10 / d`)
  is a read, the value and a write in Java's order instead of a closure. A sliced loop in a `try`
  keeps its fast copy: the check before it proves its accesses, and the other copy fails where
  Java does.
- **A null dereference** (`Facts::null_checked`) uses the checked `non_null`
  (`java::try_non_null`, or `try_non_null_message` where the message is read, see below).
- **Anything else of a standard class** (`Facts::catching`), such as `new int[n]` with a
  negative size, `System.arraycopy`, or a `compareTo` of a raw `Comparable` whose argument is of
  another class (`comparable.rs`, the bridge's cast), runs inside `java::catching(|| op)`
  (`crates/runtime/src/catching.rs`): inside it, the runtime's `throw` unwinds with the exception
  and `catching` returns it as an `Err`. A panic of anything else still unwinds.
- **A null argument of `compareTo`** on a `String` or a box is checked like a dereference:
  `try_non_null_message` gives Java's message for the argument's class, and the `Result` of
  `compare_to` passes it on with `?`. A raw `Comparable`'s call of any class's `compareTo` with a
  null argument runs inside `java::catching`, and the receiver's arm raises Java's message for its
  class, an enum's `Cannot invoke "Object.getClass()"` included (`dispatch.rs`, `compare_arm`).

`impl From<java::Exception> for Throwable` builds an object of the standard class with its
`(String)` constructor, so `getMessage` and `toString` read as Java's. Lowering declares such a
class (and that constructor) when the program has an operation that may raise it and a `finally`
or a `catch` of it or a superclass, a syntactic over-approximation; the analysis decides which a
handler really sees (`Facts::raised_classes`), and only those get a variant and a `From` arm.

Specific cases:

- **Regular expressions**: a constant pattern (and literal replacement) that compiles, or one
  from `Pattern.quote`, never throws (`java_runtime::regex::never_fails`), so a handler around
  `split`, `matches`, `replaceAll` or `replaceFirst` does not block the program. A replacement
  that is not such a literal may throw `IllegalArgumentException` or `IndexOutOfBoundsException`,
  which a `catch` sees with Java's message. A non-constant pattern may throw
  `PatternSyntaxException`, which is not a standard class, so catching it is rejected, except in
  a program translated with the JDK's sources, where the JDK's own `Pattern` runs such a call
  (`lower/regex.rs`) and throws it as Java does.
- **Formats**: a format that is not a constant may throw the `IllegalFormatException` subclasses,
  which are standard classes. The runtime builds them from the message and a `raised` flag (Java's
  constructors take the flags or width instead); a raised one's getters answer null or zero.
  `System.out.printf` prints what Java's `Formatter` writes before a specifier that fails as it
  prints (`java::print_format`). The `printf` of a `PrintStream` object formats all its text
  before printing, so one whose format may fail is rejected.
- **Number parsing of `null`**: `parseInt`/`valueOf` of `Integer`, `Long`, `Short` and `Byte`
  throw `NumberFormatException("Cannot parse null string")`. The range form
  (`parseInt(s, begin, end, radix)`) throws `NullPointerException` from `try_parse_range`
  (`codegen/parse_range.rs`).
- **Checked operations inside a closure of the translation** would put a `?` where it cannot
  leave the method; `codegen::exceptions::check_closures` rejects them on the generated text.
  An operation run inside `java::catching` whose own code also passes a failure on is rejected
  too.

- **Charset names**: `Charset.forName` and `isSupported` raise `UnsupportedCharsetException` and
  `IllegalCharsetNameException` (standard classes, with the name as the message) and, for a null
  name, `IllegalArgumentException`, which a `catch` sees as Java's does.

What the analysis has no class for (`"?"` in `analysis/exceptions/failures.rs`) is rejected
wherever a handler would see it: "catching an exception that ... may throw not supported yet".

## Static initializers

A static block or static field initializer that throws is wrapped in
`ExceptionInInitializerError` at the first use of the class, as Java does.

- **Uncaught** (the program catches none of `Error`, `Throwable`, `LinkageError`,
  `ExceptionInInitializerError`, `NoClassDefFoundError`): no `Result` anywhere. The runtime prints
  `Exception in thread "main" java.lang.ExceptionInInitializerError` and `Caused by: ...` and
  exits with 1. The analysis collapses what such an initializer raises into
  `ExceptionInInitializerError` (`INITIALIZER_ERROR`). A `finally` around a use of such a class
  is rejected, since it would see the error.
- **Caught**: `Standard::select` declares `LinkageError`, `ExceptionInInitializerError` and
  `NoClassDefFoundError`, and `lower/clinit_errors.rs` rewrites the `<clinit>` of each class with
  a static initializer:

  ```java
  static int init$state;      // 0 not started, 1 started or done, 2 failed
  static String init$failure;
  static void <clinit>() {
      if (init$state == 2) throw new NoClassDefFoundError("Could not initialize class " + <binary name>, new ExceptionInInitializerError(init$failure));
      if (init$state == 1) return;
      init$state = 1;
      try { <the original body> }
      catch (Error failure) { init$state = 2; init$failure = "Exception " + failure + " [in thread \"main\"]"; throw failure; }
      catch (Throwable failure) { init$state = 2; init$failure = "Exception " + failure + " [in thread \"main\"]"; throw new ExceptionInInitializerError(failure); }
  }
  ```

  The state is its own guard, so the rewritten `<clinit>` gets no `init_guard`; its callers
  return `Result`, and so does a method that starts by running it (`Facts::entry_init`,
  `analysis/exceptions/entry.rs`). In a program that starts a thread, the two fields are
  process-wide or thread-local with the class's other statics.
- **The JDK's static initializers** run on the JDK's own constants and flags and none of the
  program's code, so they throw only for bugs: one lets out nothing, and the `catch` clauses
  around a use of its class are not looked for in its own code. A `catch` inside a rewritten JDK
  initializer does not count as reading a `NullPointerException`'s message.

## The text of an object: `toString` and `hashCode` that throw

Making an object's text runs its `toString`. Where the generator makes the text (`"" + o`,
`println(o)`, `String.valueOf(o)` and so `append(o)`, `String.format` with its arguments one by
one), that call is like any other: what the program's `toString` throws goes on. A class whose
`toString` throws is not `Display` then; the dispatching enum of its type gets `try_text`, which
returns the exception. The `hashCode` that `Object.toString` shows, for a class without a
`toString` of its own, is treated the same way.

Elsewhere the text goes through `Display`, which cannot return an exception, so what the
`toString` throws ends the program: the text of an enum constant, the message of an exception,
`printf` (which makes all its text before printing), and the objects other library calls show.
Where a `toString` the program wrote may throw what a `catch` around such a text, or around a
call on the way to it, would catch, the program is rejected ("catching an exception that this
toString may throw not supported yet").

The JDK's own `toString`s are not looked at: they throw only for bugs (a collection changed
while it is shown), which then end the program even where a `catch` would see them. An explicit
`o.toString()` is an ordinary call, except that one of the JDK's lets out only what the
program's code it runs throws (an iterator or element of the program), and the `catch` clauses
around the call are not looked for in its own code.

Comparing, hashing, cloning or null-checking an object, `Unsafe`'s memory operations, copies and
fills of arrays, and the range of a `CharSequence` that a builder appends make no text.

## The analysis

`analysis/exceptions.rs` runs after the other analyses, since what an operation can raise
depends on their facts: a divisor the ranges prove nonzero, an index proved in bounds, a value
proved non-null raise nothing. Its predicates are shared with the generator, so an operation is
written in checked form exactly when it is in the analysis's set.

- **Points**: `throw e` raises the concrete classes under `e`'s static type; a call raises what
  escapes any method it may run (every implementation of a virtual call, constructors,
  `this(...)`, `super(...)`); a runtime operation raises its standard classes, from a table per
  builtin and expression kind (`analysis/exceptions/failures.rs`).
- **Escapes** of a method: what its points raise, minus what an enclosing `catch` in the same
  method catches, plus what its `catch` and `finally` bodies raise.
- **Observed** at a point: the classes under the enclosing `catch` types; everything, when an
  enclosing `finally` observes (it does unless it only assigns variables, fields and array
  elements with operations that cannot fail, which is invisible once the program ends); and what
  the method's callers observe of what escapes it. Both are fixpoints, unioned over override
  groups.
- **Calls that raise nothing for their arguments** (`analysis/exceptions/guards.rs`): a call with
  known arguments (literals, constants, a static assigned once from a JVM native such as
  `Unsafe.arrayIndexScale`, a new array of known length) runs its callee on those values; an `if`
  or loop whose condition is then false is skipped, and so is `new T[n]` of a known `n >= 0`. If
  nothing else raises, the call is no point. This clears the JDK's
  `ArraysSupport.exactLog2(Unsafe.ARRAY_INT_INDEX_SCALE)` and `new SetN<>()` of
  `ImmutableCollections` in static initializers.
- **Null tests and type tests narrow**: a variable tested `!= null` by an enclosing `if`, loop
  condition, `&&`, `||` or `?:`, and not assigned in the guarded code, raises no
  `NullPointerException` there. `o instanceof T` proves `o` non-null and a cast of it to `T` or a
  class above safe; `!` is followed; `if (c) return;` (or `throw`, `break`, `continue`) narrows
  the statements after it; a local initialized from a value so known is non-null. A cast cannot
  fail either when every object its value may be is under the target type (`Facts::flows`).
- **Casts that lowering inserts** (javac's conversion of an erased type variable's value) are not
  counted as `ClassCastException`.
- **Arguments before the receiver's check**: Java throws the `NullPointerException` of a call on
  null, and of a store into a null array or out of its bounds, only once the arguments (the
  stored value) are computed. Where one may have an effect, lowering computes it first into a
  local, and the receiver (the array) before it unless it is a local that nothing there assigns
  (`lower/argument_order.rs`); the message names the receiver itself.
- **Facts**: `throwing` (methods and constructors that return `Result`), `init_throwing`,
  `live_tries`, `raising_tries`, `raising`, `null_checked`, `catching`, `raised_classes`, and the
  `NullPointerException` message facts below.

Interaction with the other analyses:

- **Ranges**: the state at the start of a `catch` is the state before the `try` with every
  variable the body assigns widened to its type's full range; after the statement it is the join
  of the body's end and each `catch`'s end; the `finally` runs from the join of all of them.
- **Nullness** is flow-insensitive; a catch parameter is never null.
- **Moves**: a variable moved inside a `try` body is not moved again in a `catch` or after the
  `try`; a read in the body is a last use only when nothing after the statement reads it.
- **Arrays**: the liveness analysis does not look into a `try`, so no array is moved inside one,
  where a failure may lead to a handler that reads it.
- **Borrowing**: a `RefCell` borrow is a temporary of its statement, which `?` and `break` drop.
- **Struct-literal constructors**: a constructor that throws before its leading assignments never
  builds the object; its struct literal is the `Ok` value.

## Stack traces

Stack traces have no frames, by design: the specification of `Throwable.getStackTrace` lets a
virtual machine omit frames, and a translation has none to give (a Rust backtrace has Rust
functions, not Java methods and lines). So `getStackTrace()` is empty, `fillInStackTrace()` does
nothing and returns the exception, and `setStackTrace(frames)` stores a copy (with Java's
`NullPointerException` for a null frame), which `getStackTrace()` then returns.
`StackTraceElement` is the translator's (`Standard.java`): its constructor from class, method,
file and line (with Java's null checks), its getters and `toString`, without class loader and
module, and without `equals` and `hashCode`.

`printStackTrace()`, `printStackTrace(PrintStream)` and `printStackTrace(PrintWriter)` print what
Java's `printStackTrace` and `printEnclosedTrace` print for those traces: the exception's
`toString()`, a `\tat ` line per frame, suppressed exceptions after `\tSuppressed: ` (a tab more
per level), causes after `Caused by: `, `[CIRCULAR REFERENCE: ..]` for an exception printed
before, and frames in common with the enclosing trace elided as `... n more`. With no frames,
that is the lines of the exception, its suppressed exceptions and its causes. An uncaught
exception prints the same way after `Exception in thread "main" `. Where the program may make an
exception with more trace than its `toString()` (it uses a cause, `initCause`, `getCause`,
`addSuppressed` or `setStackTrace`), `main` first gives the runtime `Throwable.uncaughtTrace()`
(`Native::InstallTrace`), which makes the text.

The corpus compares standard output, where Java prints real frames, so
`corpus/whole-program/StackTracesGiven.java` gives every printed exception its frames with
`setStackTrace` and prints them to `System.out` and a `PrintWriter`.

Rejected: an override of `printStackTrace(PrintStream)` or `printStackTrace(PrintWriter)` (Java's
`printStackTrace()` and uncaught exceptions print through it), and an override of
`fillInStackTrace` that does more than `return this;` or `return super.fillInStackTrace();`
(Java's constructors run it). Real frames would need the translation to record them as it runs;
that is not planned for now.

## The messages of `NullPointerException`

Java's message comes from the class file: the JVM describes the instruction that met the null
and, when it can, where the null came from (`Cannot invoke "Main$Box.size()" because
"<parameter1>.next" is null`). A class compiled without `-g`, as `javac` and the source launcher
compile by default, has no local names: a parameter is `<parameterN>`, a local `<localN>`, by
slot.

**Only where a `catch` reads it.** A `catch` of such an exception that uses it other than by
throwing it on (`getMessage`, printing, passing it to a method; `Facts::npe_message_read`) makes
the checked dereferences whose exception it may get carry Java's text
(`java::try_non_null_message`): those its `try` catches, and those whose exception leaves a
method that a call in the `try` may run (`Facts::npe_message_needed`). The rest stay bare.
Adding a suppressed exception does not read either one, unless the program calls
`getSuppressed` or a `printStackTrace` to a stream or writer.

**How the names are found.** Only then does the translation compile the program a second time
with javac's code generator (`java/BytecodeNames.java`, `javac/bytecode_names.rs`), which tells
how the class file names each method (`Gen.binaryQualifier` and the erased descriptor), each
static field and each local: the slot `Gen` gives it, read after code generation, so the slots
are javac's own rather than a copy of its rules. A lambda's body is a method whose first
parameters are its captures; a local or anonymous class reads a captured local as
`this.val$x`; javac's own locals are named too (the `s$` of a string `switch`). The rest of the
message follows the JVM's rules (`analysis/npe_messages.rs`): a field chain up to five steps
back, an element with its index when that is a constant, a local or a call (`[...]` otherwise),
no `because` for the value of `c ? a : b`, no message for a pattern switch's
`Objects.requireNonNull`, and an assigned value followed back to its source
(`(s = a).next` fails `because "a" is null`).

**The JDK's code** is taken to be compiled with local names, as the JDK is, so its messages name
its locals (`because "original" is null`). A call of the JDK's is named by the erasure of its
receiver's type and its descriptor, with type variables erased as javac does; a method that
`Object` declares stays `Object`'s, and an enum constant body's call of its own method is named
by the body's anonymous class (`java.time.temporal.IsoFields$Field$1.rangeRefinedBy(...)`), whose
binary name the tree gives. Also covered: `String` methods by the overload javac chose,
unboxing as the call it is (`java.lang.Integer.intValue()`), the JDK's constructors that fail
inside (`new String(s)`, `new StringBuilder(s)`), a static by the class that declares it, an
inner class's outer object as `this$0`, an enum field kept as a table (`lower/enum_tables.rs`) as
the field it is, and a string `switch`'s temporary as `<localN>` from the slots javac would give
(`javac::jdk_switch_temps`). The receiver of an unbound method reference has no message, as the
JVM's hidden frame gives none. The translator's own `java.lang` classes (`Standard.java`) have no
known message from their code; where the JDK's code has a dereference that may fail, they throw
the JDK's message themselves. The overlays of `java.lang` (`Module`, `ModuleLayer`,
`TranslatedModules`, `TranslatedProperties`) get the messages of their own code.

**Dereferences that never fail need no message**: besides the null tests of the analysis, a
variable tested as it is assigned (`(k = e.getKey()) != null && k.equals(key)`), one that
`if (v == null) v = ..;` sets, and a static that the first statements of its class's static
initializer set to a non-null value and nothing else assigns (`Facts::initialized_statics`), when
nothing those statements run reads it first. A branch of an `if` on a call of a method that only
returns a constant (the translator's `Thread.isVirtual()`) is never taken.

**Checked against Java 26** (`CHECKED_JAVA` in `javac/bytecode_names.rs`). With another Java the
texts may differ, and a program whose `catch` reads one is not translated.

**Rejected** where a `catch` may read a message that is not certain: a dereference of a parameter
its method assigns (the JVM names it `<localN>` or `<parameterN>` depending on the path), of a
local declared in a `finally` (javac copies the block, each copy with its own slots) or in an
instance initializer (copied into every constructor), of a JDK local the translation may have
renamed, of the enclosing instance of a nested class, a dereference in the code of `java.lang`,
a failure of the runtime without the checked form (an unboxing the code does not write as a
call, a library call given null other than a `compareTo`'s argument), a dereference in the body of a method reference's class, and a
field of a field of an enclosing instance.

## Differences from Java

- Java gives the `IllegalArgumentException` of a self-suppression the exception itself as its
  cause; the translation leaves the cause `null`.
- When a superclass's static initializer failed, the superclass's initialization runs before the
  subclass's state check and does not set the subclass's state, so a later use of the subclass
  throws the superclass's `NoClassDefFoundError` where Java throws the subclass's.
- The analysis takes the `Error` clause of a rewritten `<clinit>` to rethrow any `Error`, so a
  method may return a `Result` where it need not.
- With heap pollution (a raw or unchecked use that puts a value of another class in a generic
  place), Java's inserted cast fails and a `catch` may see it; the translation's ends the
  program instead.
- `toString`s of the JDK that throw (only for bugs) end the program even inside a `try` that
  would catch them.

## Not done

Rejected, each with a message naming the construct:

- catching a class that is neither a standard class nor translated (in a single file, for
  example `StackOverflowError` or `PatternSyntaxException`); the translation never raises
  `StackOverflowError` or `OutOfMemoryError`;
- a `toString` of the program that throws what a `catch` around an object's `Display` text would
  catch;
- a constructor of an enum that throws something a caller catches, and a caught call of `main`;
- a `finally` that would see an uncaught static initializer's failure;
- try-with-resources over a resource without a translated `close()`;
- an observed failure that the analysis cannot name, or that would be checked inside a closure
  of the translation;
- an uncertain `NullPointerException` message that a `catch` reads;
- overrides of `printStackTrace` of a stream or writer, and a `fillInStackTrace` that does more
  than return the exception.
