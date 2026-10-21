# Exceptions: `throw`, `try`/`catch`/`finally`, the standard exception classes

This plan builds on the object model (`docs/object-model.md`) at its milestone 2: class trees
with several concrete classes are enums. It is kept up to date as decisions change (see
"Deviations" at the end).

Decided by the user: exceptions are `Result`-based. A method that can throw returns
`Result<T, Throwable>`, `?` propagates, and a method that cannot throw keeps its plain
signature. Everything else is decided here.

## 0. The decisions in one page

1. **Exception classes are classes of the object model.** A program's own exception classes
   (`class InsufficientFunds extends Exception`) are ordinary classes; the standard ones the
   program uses (`Throwable`, `Exception`, `RuntimeException`, `IllegalArgumentException`, ...)
   are *synthetic* classes that lowering adds to the program (`lower/exceptions.rs`), with
   Java's names, the constructors the program calls and the methods of `Throwable` it calls,
   their bodies written as small syntax trees. So the object model's machinery (fields structs,
   enums, dispatch, `Display`, identity) applies to exceptions unchanged, and a user class that
   overrides `getMessage` or `toString` is dispatched to like any override.
2. **The standard classes are provided by the translator, not read from the JDK.** They are in
   `program/reach.rs`'s `RUNTIME_CLASSES`, so reachability never reads `Throwable.java` (stack
   traces, serialization, suppressed exceptions, natives) and a single file and a whole
   program translate them the same way. An exception class of the JDK that the table does not
   list is translated from its source in whole-program mode (they are small classes that call
   `super(...)`), and rejected in a single file.
3. **`E` is the enum `Throwable`**, the representation of `java.lang.Throwable`: one variant per
   exception class that the program creates or that the runtime raises where the program
   observes it. It is an enum even when it has one variant, so that it is a type of the
   program and the runtime's failures convert into it (`impl From<java::Exception> for
   Throwable`, which `?` uses). The other standard types with several concrete classes under
   them (`Exception`, `RuntimeException`, ...) declare no members of their own, so they are
   represented by `Throwable` too rather than by enums of their own: no conversions are needed
   between them, and a `catch (RuntimeException e)` binds the value it matched as it is.
4. **The fields of `Throwable`** are a fields struct, `ThrowableFields { message: Option<String>,
   cause: Option<Throwable> }`, the field `cause` only when the program uses causes. A standard
   class that declares nothing, has no objects of its own and whose constructors only pass their
   parameters to `super(...)` has no fields struct: the struct of a class under it embeds
   `ThrowableFields` directly (`struct InsufficientFunds { base: ThrowableFields }`), and its
   `super(m)` builds that struct.
5. **Which methods throw** is an analysis (`analysis/exceptions.rs`): a fixpoint over calls
   (every implementation a virtual call may run, constructors, `this(...)`, `super(...)`) of what
   each method may let escape, and of what the callers of each method observe. A method returns
   `Result` exactly when some caller observes an exception that may escape it: a call inside a
   `try` whose `catch` matches it or whose `finally` observes it, or a call in a method whose own
   callers observe it. The methods of an override group agree.
6. **Failures of the runtime become values only where they are observed.** Division by zero,
   array indexes and sizes, null dereferences, parsing and the string indexes raise exceptions in
   the runtime today by printing them and exiting (`java::throw`). That stays the translation
   wherever no `catch` or `finally` could see the failure. Where one could, the operation uses the
   runtime's checked form (`java::try_div(a, b)?`), which returns `Result<_, java::Exception>`. An
   operation the runtime has no checked form for, in a place where its failure is observed, is
   rejected.
7. **`try` is a labeled block** that evaluates to `Result<(), Throwable>`. Inside it, a failure
   leaves the block with `java::or_break!(call, 'attempt)` instead of `?`; the `catch` clauses are
   a `match` on the error's variants; `finally` runs after the match on every path. `return`,
   `break` and `continue` inside a `try` that has no `finally` are Rust's own; with a `finally`,
   the block evaluates to `java::Completion` (normal, return, jump or throw) and the jump happens
   after the `finally` body.
8. **Uncaught exceptions** end the program as Java's do: `Exception in thread "main" ` and the
   exception's `toString()` on standard error, a `Caused by: ...` line per cause, exit code 1.
   Stack frames are not printed (they cannot match). `printStackTrace()` prints the same lines
   without the thread prefix.
9. **Static initializers return `Result` only where a `catch` observes them**: a static block, or
   a static field initializer, that throws is wrapped in `ExceptionInInitializerError` as Java
   does, at the first use of the class. The uncaught case translates without a `Result`: the
   runtime prints `Exception in thread "main" java.lang.ExceptionInInitializerError` and
   `Caused by: ...` on standard error and exits 1, with the same stdout as Java. A program that
   catches one of `Error`, `Throwable`, `LinkageError`, `ExceptionInInitializerError` or
   `NoClassDefFoundError` gets the state machine of section 4, step 8: the first use throws the
   failure, and every later use throws `NoClassDefFoundError`. A `finally` around the use is
   rejected unless the program catches one of those classes.
10. **Rejected for now**, each with a message naming the construct: observing the message of a
    `NullPointerException` the runtime raises (Java's helpful message names bytecode locals;
    `getMessage()` in a `catch` that such an exception reaches is rejected, while the message of
    one that code creates, as `Objects.requireNonNull(x, "msg")` does, is its own),
    catching `StackOverflowError`, `OutOfMemoryError` or `VirtualMachineError` (the translation
    cannot raise them), a `toString` that throws an exception someone observes (it is
    `Display`), `getStackTrace`, `setStackTrace`, `fillInStackTrace` and `initCause`.

## 1. The representation

### 1.1 A program with its own exception

```java
class Bank {
    static class InsufficientFunds extends Exception {
        InsufficientFunds(String msg) { super(msg); }
    }
    static class Account {
        long balance;
        void withdraw(long amount) throws InsufficientFunds {
            if (amount > balance) throw new InsufficientFunds("has only " + balance);
            balance -= amount;
        }
    }
    static void transfer(Account from, Account to, long amount) {
        try {
            from.withdraw(amount);
            to.balance += amount;
        } catch (InsufficientFunds e) {
            System.out.println("failed: " + e.getMessage());
        } finally {
            System.out.println("done");
        }
    }
}
```

```rust
#[derive(Clone)]
enum Throwable {
    InsufficientFunds(Rc<BankInsufficientFunds>),
}

struct ThrowableFields {
    message: Option<String>,
}

impl ThrowableFields {
    fn new(message: Option<String>) -> ThrowableFields {
        ThrowableFields { message }
    }

    fn get_message(&self) -> Option<String> {
        self.message.clone()
    }
}

struct BankInsufficientFunds {
    base: ThrowableFields,
}

impl BankInsufficientFunds {
    fn new(msg: String) -> BankInsufficientFunds {
        BankInsufficientFunds { base: ThrowableFields::new(Some(msg)) }
    }
}

impl BankAccount {
    fn withdraw(&mut self, amount: i64) -> Result<(), Throwable> {
        if amount > self.balance {
            return Err(Throwable::InsufficientFunds(Rc::new(BankInsufficientFunds::new(format!(
                "has only {}",
                self.balance
            )))));
        }
        self.balance -= amount;
        Ok(())
    }
}

fn transfer(from: &Rc<RefCell<BankAccount>>, to: &Rc<RefCell<BankAccount>>, amount: i64) {
    let attempt: Result<(), Throwable> = 'attempt: {
        java::or_break!(from.borrow_mut().withdraw(amount), 'attempt);
        to.borrow_mut().balance += amount;
        Ok(())
    };
    let attempt = match attempt {
        Err(Throwable::InsufficientFunds(e)) => {
            java::println!("failed: {}", java::or_null(e.base.get_message()));
            Ok(())
        }
        other => other,
    };
    java::println!("done");
    if let Err(e) = attempt {
        java::uncaught(e);
    }
}
```

`withdraw` returns `Result` because its call in `transfer` is inside a `try` that catches what
it throws. `transfer` does not: nothing escapes it that a caller observes, so the `Err` arm left
after the `finally` (which cannot happen here, but the analysis does not need to know that) ends
the program as Java's uncaught exception would. Without the `finally`, the handlers are a plain
`match attempt { Ok(()) => {} Err(Throwable::InsufficientFunds(e)) => { .. } Err(e) => .. }`.

### 1.2 The standard classes

`lower/exceptions.rs` holds a table of the standard classes: their qualified name, their
superclass, which of the four `Throwable` constructor shapes they have (`()`, `(String)`,
`(String, Throwable)`, `(Throwable)`) and any other constructor (`(int)` of the index
exceptions, and the `AssertionError` constructors with a detail of any type: `(Object)`, which
takes a `Throwable` as the cause, and the primitive ones; `MissingResourceException` has
`(String, String, String)` and `UncheckedIOException` has `(String, IOException)` and
`(IOException)`, and `ClassNotFoundException` has `getException`):

```
java.lang.Throwable
  java.lang.Exception
    java.lang.RuntimeException
      java.lang.ArithmeticException, java.lang.ArrayStoreException,
      java.lang.ClassCastException, java.lang.IllegalArgumentException
        (java.lang.NumberFormatException), java.lang.IllegalStateException,
      java.lang.IndexOutOfBoundsException (java.lang.ArrayIndexOutOfBoundsException,
        java.lang.StringIndexOutOfBoundsException), java.lang.NegativeArraySizeException,
      java.lang.NullPointerException, java.lang.UnsupportedOperationException,
      java.lang.SecurityException, java.util.NoSuchElementException
        (java.util.InputMismatchException), java.util.ConcurrentModificationException,
      java.util.EmptyStackException, java.util.MissingResourceException,
      java.io.UncheckedIOException
    java.lang.InterruptedException, java.lang.CloneNotSupportedException,
    java.lang.ReflectiveOperationException (java.lang.ClassNotFoundException,
      java.lang.NoSuchMethodException, java.lang.NoSuchFieldException,
      java.lang.IllegalAccessException, java.lang.InstantiationException),
    java.io.IOException (java.io.FileNotFoundException, java.io.EOFException,
      java.io.UnsupportedEncodingException)
  java.lang.Error
    java.lang.AssertionError, java.lang.LinkageError
      java.lang.ExceptionInInitializerError, java.lang.NoClassDefFoundError
```

Before the program's methods are lowered, lowering scans javac's tree for every symbol and type
that names one of these classes, and declares the synthetic class for each one named, with its
superclasses. A synthetic class is abstract unless the program creates its objects (`new`) or
the runtime may raise it (section 3), so the enum of `Throwable` only has the variants it
needs. Of each class it declares the constructors the program calls (and those that the
runtime's failures and `super(...)` chains need), and of `Throwable` the methods the program
calls, with the methods those call: `getMessage`, `getLocalizedMessage`, `getCause`,
`toString`, `printStackTrace()`. Their bodies:

```java
Throwable(String message) { this.message = message; }
Throwable(String message, Throwable cause) { this.message = message; this.cause = cause; }
Throwable(Throwable cause) { this.message = cause == null ? null : cause.toString(); this.cause = cause; }
String getMessage() { return message; }
String getLocalizedMessage() { return getMessage(); }
Throwable getCause() { return cause; }
String toString() { String m = getLocalizedMessage(); return m != null ? <class name> + ": " + m : <class name>; }
void printStackTrace() { System.err.println(this); for (Throwable c = getCause(); c != null; c = c.getCause()) System.err.println("Caused by: " + c); }
```

and every other standard constructor is `X(params) { super(params); }`, except
`ExceptionInInitializerError(Throwable thrown)`, which is `super(null, thrown)`, and
`ExceptionInInitializerError(String)`, which is `super(message, null)`, as Java's are. `<class name>` is a new
builtin, `Builtin::ClassName`: Java's binary name of the object's class (`java.lang.Exception`,
`Bank$InsufficientFunds`), a string literal for an object of a known class and the generated
`class_name()` of an enum otherwise. Java's `cause` field starts as the object itself, meaning
"not set yet", for `initCause`; since `initCause` is rejected, `null` is the same.

The Rust names of the synthetic classes are their simple names (`IllegalArgumentException`);
a program class whose name would clash with one the program needs is rejected.

### 1.3 Values of exception types

| Java type of the value | Rust |
| --- | --- |
| a class with one concrete class under it | that class's object, `Rc<C>` (or `Rc<RefCell<C>>` when the family is mutable) |
| `Throwable`, and a standard class with several concrete classes under it | `Throwable` |
| a program class with several concrete classes under it | the enum of that class, as for any class tree |

`throw e` and every `Err` convert the value to `Throwable` (an upcast: the variant wrap of an
object, `to_throwable()` from a program class's enum). A `catch` clause of type `T` matches the
variants of the classes under `T`; it binds the variant's object when `T` has one concrete
class (`Err(Throwable::InsufficientFunds(e))`), the matched value itself when `T` is
represented by `Throwable` (`Err(e @ (Throwable::A(_) | Throwable::B(_)))`), and the value
converted to `T`'s enum otherwise (`e.to_bank_error()`, a conversion whose other arms are
unreachable). Multi-catch is one arm whose pattern is the union; a catch type no variant is
under matches nothing and is not written. The order of the arms is the order of the clauses,
which is Java's (the first matching clause runs).

### 1.4 Methods that throw

```java
static int parse(String s) throws Bad { if (s.isEmpty()) throw new Bad("empty"); return Integer.parseInt(s); }
```

```rust
fn parse(s: &str) -> Result<i32, Throwable> {
    if s.is_empty() {
        return Err(Throwable::Bad(Rc::new(Bad::new("empty"))));
    }
    Ok(java::try_parse_int(s)?)
}
```

(`try_parse_int` is the checked form only if some caller observes a `NumberFormatException`;
otherwise it stays `java::parse_int(s)`.) In a method that returns `Result`:

- `return v` is `return Ok(v)`, the tail value is `Ok(v)`, and a body that can complete normally
  ends with `Ok(())`;
- `throw e` is `return Err(..)`;
- a call of a method that returns `Result` is `call?`, and an operation whose failure is
  observed is `java::try_op(..)?`.

In a method that does not, a call of a method that returns `Result` (because other callers
observe it) is `call.unwrap_or_else(java::uncaught)`: nothing that escapes here is observed,
so the exception ends the program at once, which is what propagating it would do. `throw e`
outside any `try` in such a method is `java::uncaught(e)`.

A constructor that throws returns `Result<C, Throwable>` (its struct literal or object is the
`Ok` value), and `new C(..)` is `C::new(..)?` inside the wrapping. `toString` methods that
throw an observed exception are rejected: `Display::fmt` cannot return it. The dispatcher of
an override group whose members return `Result` returns it too and forwards it unchanged.

### 1.5 `try`, `catch`, `finally`

The general form, for `try { B } catch (X e) { C } finally { F }` in a method that returns
`Result`:

```rust
let attempt: Result<(), Throwable> = 'attempt: {
    B          // a failure here: break 'attempt Err(e), through java::or_break!
    Ok(())
};
let attempt = match attempt {
    Err(Throwable::X(e)) => 'handler: {
        C      // a failure here: break 'handler Err(e)
        Ok(())
    },
    other => other,
};
F
attempt?;
```

- Without a `finally`, the handlers are the arms of `match attempt`, a failure inside `C`
  propagates as anywhere else in the method, and the last arm passes on what no clause
  matches: `Err(e) => return Err(e)`, or `java::uncaught(e)` in a method that does not return
  `Result`, or a break to the enclosing `try`'s label.
- When `B` ends with `return v` (and so cannot complete normally), the block evaluates to
  `Result<T, Throwable>` and its tail is `Ok(v)`; the `Ok(v)` arm returns `v`. This keeps the
  common `try { return parse(s); } catch (..) { return -1; }` free of placeholders.
- With a `finally`, a `return`, `break` or `continue` inside `B` or `C` that leaves the `try`
  must run `F` first: the blocks then evaluate to `java::Completion<T, Throwable>`
  (`Normal`, `Return(v)`, `Jump(n)`, `Throw(e)`), the jump is a `break 'attempt
  java::Completion::Jump(n)`, and after `F` a `match` performs it (`return v`, `break 'outer`,
  `continue`). When `F` itself completes abruptly (it returns, breaks or throws), that is what
  happens and the pending completion is dropped, as in Java.
- Rust allows no unlabeled `break` or `continue` inside a labeled block (E0695), so a jump out
  of a `try` body always names its loop's label; the `try` is a frame of `codegen/switch.rs`
  that is not a loop.
- A local declared before the `try` without an initializer and assigned inside it cannot be
  `let x;` in Rust, whose definite-initialization check does not follow the `Err` path; it is
  declared with Java's default value instead (which no path reads).

`java::or_break!(expr, 'label)` is a runtime macro: `match expr { Ok(v) => v, Err(e) => break
'label Err(From::from(e)) }`, the `From` turning a runtime failure into `Throwable`.

### 1.6 The runtime

```rust
/// An exception that the runtime raises: Java's class name and the message.
pub struct Exception { pub class: &'static str, pub message: Option<String> }
pub fn uncaught<T>(e: impl fmt::Display) -> T;                // prints and exits with 1
pub enum Completion<T, E> { Normal, Return(T), Jump(usize), Throw(E) }
macro_rules! or_break { .. }
pub fn try_div<T: Integer>(a: T, b: T) -> Result<T, Exception>;   // and try_rem
pub fn try_get / try_set / try_update / try_filled / try_non_null / try_parse_int / ...
```

Each checked form computes what the existing function computes, and the existing function
becomes `try_x(..).unwrap_or_else(uncaught)` where that keeps it as fast (the exiting forms
stay the default in generated code). The program's `impl From<java::Exception> for Throwable`
matches on the class name and builds the variant with the message; a class with no variant
(which the analysis rules out) ends the program as uncaught.

## 2. The analysis

`analysis/exceptions.rs` runs after the other analyses, since what an operation can raise
depends on their facts (a division by a divisor the ranges prove nonzero raises nothing, an
index the ranges prove in bounds raises nothing, a value nullness proves non-null raises no
`NullPointerException`). The predicates are shared with the generator, so an operation is
written with the checked form exactly when it is in the analysis's set.

- **What a point raises**: `throw e` raises the concrete classes under `e`'s static type; a call
  raises what escapes any method it may run; an operation of the runtime raises its standard
  classes (a table per `Builtin` and per expression kind: `/` and `%` on integers
  `ArithmeticException`, an index `ArrayIndexOutOfBoundsException`, `new T[n]`
  `NegativeArraySizeException`, a dereference of a value that may be null
  `NullPointerException`, `Integer.parseInt` `NumberFormatException`, `charAt` and `substring`
  `StringIndexOutOfBoundsException`, ...). A builtin with no entry raises "anything", which is
  rejected wherever it is observed.
- **What escapes a method**: what its points raise, minus what an enclosing `catch` of the same
  method catches, plus what the `catch` and `finally` bodies raise. A static initializer's
  escapes are wrapped in `ExceptionInInitializerError`: the uncaught case collapses them into that
  class (`INITIALIZER_ERROR`), and a rewritten initializer (section 0.9) raises what its state
  machine rethrows. A method that starts by running an initializer (`Facts::entry_init`) raises
  what that initializer raises at its start (`analysis/exceptions/entry.rs`). An instance field
  initializer's escapes escape each constructor that runs it (`analysis/exceptions/field_inits.rs`).
- **What is observed at a point**: the classes under the types of the enclosing `catch`
  clauses, everything when an enclosing `finally` observes (it does unless it only assigns
  variables, fields and array elements with operations that cannot fail: what it does to memory
  is then invisible once the uncaught exception ends the program), and what the callers of the
  method observe of what escapes it (`observed(m)`). `observed(m)` is the fixpoint of: for every
  call of `m` at point `p`, what is observed at `p` of what escapes `m`; over override groups,
  the union of the members'.
- **Facts**: `throwing: HashSet<usize>`, the methods (and constructors) that return `Result`;
  `raising: HashSet<ExprId>`, the operations written with the checked form;
  `exception_classes: HashSet<usize>`, the standard classes the runtime may raise where observed
  (they get a variant and a `From` arm).

### 2.1 Interaction with the other analyses

A `?` or `break 'attempt` is a new way out of a statement. The analyses that follow the flow of
a method must see it:

- **Ranges** (`analysis/ranges.rs`): the state at the start of a `catch` is the state before the
  `try` with every variable the `try` body assigns widened to its type's full range; after the
  statement it is the join of the body's normal end and each `catch` body's end; `finally` runs
  from the join of all of them, and a `finally` that assigns a variable widens it in the states
  that `break` and `continue` carry out of the `try`. The return value of a method that returns
  `Result` is the `Ok` value's range, as before.
- **Nullness** is flow-insensitive, so nothing changes, except that a catch parameter is never
  null.
- **Moves** (`analysis/moves.rs`): a variable moved inside a `try` body is not moved again in a
  `catch` or after the `try`; a read inside a `try` body is a last use only when nothing after
  the statement (including the handlers and `finally`) reads it.
- **Borrowing and ownership**: a `RefCell` borrow is a temporary of its statement, which `?`
  and `break` drop like any other exit; nothing changes. An exception object is converted to
  `Throwable` when thrown, so it is never owned, and exception classes are immutable unless the
  program writes their fields after construction (then the family is mutable together).
- **Struct-literal constructors**: a constructor that throws before its leading assignments
  never builds the object; its struct literal is the `Ok` value.
- **Statics**: an `init_x()` call and a lazy static's initializer use the exiting forms (see
  0.9). A rewritten `<clinit>` is its own guard, so it gets no `init_guard`; the call that runs it
  is passed on like any other call that can throw.

## 3. Steps

Each step is mergeable on its own, with tests green and the corpus extended.

1. **This document.**
2. **The runtime**: `java::Exception`, `uncaught`, `or_break!`, `Completion`, and the checked
   forms of division, remainder, array access and creation, `non_null`, number parsing,
   `charAt` and `substring`, with unit tests.
3. **`throw` and `try`/`catch` with the standard classes and the program's own exception
   classes**: the synthetic classes, the analysis, `Result` methods and `?`, handlers, uncaught
   exceptions, `getMessage`, `toString`, catch order and subclass matching, multi-catch,
   rethrow, nested `try`. Corpus: `features/ThrowAndCatch.java`, `features/ExceptionClasses.java`,
   `features/CatchOrder.java`, `features/UncaughtException.java`, `programs/BankTransfers.java`.
4. **`finally`** with the rules for `return`, `break`, `continue` and exceptions in `try`,
   `catch` and `finally`. Corpus: `features/Finally.java`, `features/FinallyOverrides.java`.
5. **Failures of the runtime as values**. Corpus: `features/RuntimeExceptionsCaught.java`,
   `features/ArrayFailuresCaught.java`, `programs/RpnCalculatorErrors.java`.
6. **Causes, `getCause`, `printStackTrace`, constructors that throw**. Corpus:
   `features/ExceptionCauses.java`, `features/ThrowingConstructors.java`,
   `features/ThrowingConstructorChains.java`, `features/ThrowingConstructorsRegistered.java`.
7. **try-with-resources** over the program's own `AutoCloseable` classes, declared or as an
   existing variable (`try (r)`, `try (this.r)`). Corpus: `features/TryWithResources.java`,
   `features/TryWithExistingLocal.java`, `features/TryWithExistingField.java`,
   `features/TryWithExistingSuppressed.java`.
8. **The leftovers**: instance field initializers and constructors that throw inside a `try`,
   static initializer failures caught as `ExceptionInInitializerError` and `NoClassDefFoundError`,
   the two checks of `addSuppressed`, and the null-string parse. Corpus:
   `features/ThrowingConstructorsCaught.java`, `features/ThrowingInitializersInSubclasses.java`,
   `features/StaticInitializerErrorCaught.java`, `features/AddSuppressedChecks.java`,
   `features/ParseNullStrings.java`, `features/ParseRangeNullAndIndices.java`.

The survey's "exceptions" feature stops counting `try` and `throw` once step 4 is in; a class
extending a standard exception class stops counting as "extending classes outside the sources"
with step 3.

## 4. Deviations

Decisions that changed during the implementation, step by step.

### Step 3: `throw`, `try`/`catch`

- **The standard classes are Java source**, `crates/translator/java/Standard.java` (package
  `standard`), rather than syntax trees built by hand: `build.rs` prints its tree with
  `JavaTree` at build time, and lowering renames the package to the real one (`java.lang`, or
  `java.util` for `NoSuchElementException` and `ConcurrentModificationException`) and declares
  the classes and members that the program uses, with the members those use. A class is abstract
  unless the program creates its objects. The table of section 1.2 is that file.
- **Pass-through classes keep their fields structs** (`IllegalArgumentException { base:
  RuntimeExceptionFields }`, which embeds `ExceptionFields`, which embeds `ThrowableFields`)
  instead of embedding `ThrowableFields` directly (section 0.4): it is the object model's
  ordinary layout and costs nothing at run time.
- **No `From<java::Exception>` yet**: the runtime's failures are not values until step 5, so
  catching one is rejected wherever the analysis finds that a `catch` would see it (with the
  operation named: "catching the `ArithmeticException` that this division may throw").
  Operations of the library are classified per builtin (`library_failure` in
  `analysis/exceptions.rs`); one whose failures are not known exactly counts as throwing any
  standard class.
- **`throw` where nothing catches** is `java::throw_uncaught(e)`, a function that does not
  return, and a call of a method that throws is `call.unwrap_or_else(java::uncaught)` there.
  `Throwable` then gets its `Display`, which is its `toString`.
- **A `try` body that cannot complete normally** is a block of `Result<Infallible, Throwable>`:
  a `return` inside it is Rust's own `return`, and the `match` has no `Ok` arm, instead of the
  `Ok(v)` arm of section 1.5.
- **A `catch` of a class of the program with an enum of its own** converts the matched
  `Throwable` with an inline `match` of the arm's variants (`let e = match caught {
  Throwable::A(o) => AppError::A(o), .. }`), or with the one variant directly; a `catch` type
  under which nothing that the body throws falls is not written. The flows of the object model
  make every exception class a value of `Throwable`, and the values of `Throwable` under a
  `catch` type values of that type, so the enums have the variants the arms need.
- **A local declared without a value and assigned in a live `try` body** starts at Java's
  default value under `#[allow(unused_assignments)]`; such a local of a type without a
  default value here (an object, an array) is rejected.
- **Still rejected** (each with a message naming it): `finally` (step 4, now done),
  try-with-resources (step 7), constructors and `toString` methods that throw something a caller catches (step 6),
  a call of `main` that is caught, an enum constant initializer that throws an exception of
  the program, a static or field initializer that a `catch` or `finally` could observe (item 9),
  a program class named like a standard class it uses, and a call that throws inside
  an expression the translation writes as a closure (checked on the generated text).
  A `throw` of a value that may be null throws `NullPointerException`, as Java does.

### Step 4: `finally`

- **Three forms, the plainest that is equivalent.** A `try` that nothing leaves early (no
  `return` or jump out of its body or `catch` clauses, and no exception of the program, which
  the analysis records as `Facts::raising_tries`) is its body as a plain block followed by the
  `finally`'s statements. One that only an exception leaves early is the `Result` form of
  section 1.5, ending with `attempt?;` (or `java::or_break!`, or
  `attempt.unwrap_or_else(java::uncaught)`). One that a `return` or a jump leaves is the
  `java::Completion` form; whether a `return` or a jump leaves it is read off its statements.
- **`java::Completion<T, E, J>` has the jump's type as a parameter**: a block that no jump
  leaves has `Infallible` for `J`, one that no `return` leaves `Infallible` for `T`, so that
  the `match` after the `finally` has only the arms that can happen (Rust 1.82's exhaustive
  patterns allow leaving out the arms of uninhabited variants). A program without exceptions
  has `Infallible` for `E` too, and no `Throw` arm.
- **When neither the body nor a `catch` clause completes normally**, the `Result` form's block
  is `Result<Infallible, Throwable>` and ends with `let Err(e) = attempt;` and the `throw`, and
  the `Completion` form's `Normal` arm is `unreachable!`, so that the code after the `finally`
  diverges as Java's does (a method's last statement needs no value after it).
- **Labels are given when used**: the label of a `try` block or a `catch` clause's block is
  written when something breaks out of it (`Gen::open_block`, `Gen::name_frame`), so that no
  label is unused. A `catch` clause of a `try` with a `finally` is a block `'handler: { .. }` that
  evaluates like the body, in a `let attempt = match attempt { Err(..) => 'handler: { .. },
  other => other };`.
- **A jump through several `finally`s** goes to the innermost one; its `match` makes the jump
  again, which the next `finally` out intercepts, so each runs in order. The jumps of one `try`
  are numbered in the order they are written, and the last arm is `Jump(_)`.
- **A `return` through a `finally` takes its value first**, as Java does; a `String` variable
  returned there is cloned rather than moved, since the `finally` may read it.
- **A local declared without a value and assigned in the body or a `catch` clause** of a `try`
  with a `finally` that is not a plain block starts at Java's default value, as in a live `try`
  body.
- **`java.io.IOException` and `java.io.FileNotFoundException`** are standard classes now, and
  `getClass().getSimpleName()` is `Builtin::ClassName { simple: true }`, for the target program
  `CheckedExceptions`, which step 5 (catching a malformed number) still blocks.
- **`yield` and a constructor's `return` through a `finally`**: the value is kept in a slot
  outside the `try`, the `finally` body runs, and then the switch expression yields it or the
  constructor returns its object, as Java does. A failure of the runtime under a `finally` stays
  rejected until step 5, as under a `catch`.

### Step 5: failures of the runtime as values

- **Which operations**: a division or remainder (also compound), an array element read, write,
  compound update or `++`/`--`, `charAt`, `substring`, and `Integer.parseInt` and the other
  parse methods (with a radix too). Where a `catch` or a `finally` may see one of them fail
  (`Facts::raising`), the operation is the runtime's checked form (`java::try_div`,
  `java::try_put`, `java::try_modify`, `java::try_parse_number`, ...), whose `Err` goes on as a
  thrown exception does (`?`, `java::or_break!` or `unwrap_or_else(java::uncaught)`); everywhere
  else it stays the plain form that ends the program. `Gen::may_fail` picks the form.
- **`impl From<java::Exception> for Throwable`** turns the runtime's failure into an object of
  its standard class, built with the class's `(String)` constructor from the runtime's message,
  so `getMessage` and `toString` read as Java's. `?` and `java::or_break!` convert with it.
- **The classes are declared at lowering**: the standard classes are given to the program as
  Java source before the analysis runs, so the lowering adds a raised class (and its `(String)`
  constructor, as an instantiated member) when the program has an operation that may raise it
  and a `finally` or a `catch` of it or a superclass. This is a syntactic over-approximation;
  the analysis decides which of them a handler actually sees (`Facts::raised_classes`), and
  only those are live, flow to `Throwable` and appear in the `From` impl.
- **A literal pattern that compiles** makes `split`, `matches`, `replaceAll` and
  `replaceFirst` (with a literal replacement) calls that never throw
  (`java_runtime::regex::never_fails`), so a handler around them does not block the program.
- **Still rejected**, with "catching the `X` that this ... may throw": a null dereference
  (`NullPointerException`, whose checked form would wrap every `non_null`), an array creation
  with a negative size, a `ClassCastException` of a downcast, the library calls that may fail
  in more than one way (a `StringBuilder` index, a pattern that is not a literal, `repeat`), and
  any checked operation that would land inside a closure of the translation
  (`check_closures`), where the `?` would not leave the method.
- **Plain arrays** (main's `Vec`s and `Vec`s of `Vec`s) have checked forms of their helpers too
  (`java::try_at`, `try_put`, `try_modify`, `try_row`, `try_row_mut`, `try_position_of`,
  `try_assign`). An element update whose value may fail (`counts[i] += 10 / d`,
  `row[j] += grid[r][j]`) is a read, the value and a write one after the other, in Java's
  order, instead of a closure. A sliced loop inside a `try` keeps its fast copy: the check
  before it proves its accesses, and the other copy fails where Java does.
- **The array analyses and `try`**: the liveness analysis does not look into a `try` (every
  array it mentions is live throughout it, as for a `switch`), so no array is moved inside one,
  where a failure may lead to a `catch` or a `finally` that reads it.

### Step 6: causes and constructors that throw

- **Causes and `printStackTrace` came with step 3**: `Throwable` in `Standard.java` keeps its
  cause, `getCause` returns it, a cause-only constructor takes the cause's `toString` as its
  message, and `printStackTrace` prints the chain with `Caused by: ` lines and no stack frames
  (the corpus compares standard output and the exit code, not standard error). This step adds
  the corpus program `features/ExceptionCauses.java`.
- **A constructor that throws** something a caller catches returns `Result<T, Throwable>`, as a
  method does: `new` passes the failure on with `?` (or `java::or_break!`), so does a `this(...)`
  or a `super(...)` that throws, and the object is returned as `Ok(obj)`. A constructor that only
  delegates to one that throws returns the other's `Result` as it is. A partly built object is
  dropped, which Java's program could not see either.
- **Null tests guard the null checks**: a variable tested with `!= null` by an enclosing `if`,
  loop condition, `&&`, `||` or `?:`, and not assigned in the code the test guards, cannot be
  null there, so a `catch` around it does not see a `NullPointerException` from it
  (`next != null && next.value == value`). The nullness facts themselves hold for the whole
  method.
- **So do `instanceof` tests and early returns** (for the JDK's `equals` methods, which a
  `catch (ClassCastException | NullPointerException unused)` in `AbstractMap.equals` and
  `AbstractSet.equals` observes): `o instanceof T` proves `o` not null and a cast of it to `T`
  or a class above `T` safe, `!` is followed, `if (c) return;` (or `throw`, `break`,
  `continue`) narrows the statements after it in its block, and a local initialized from a
  value so known (`Point p = (Point) o;`) is not null (`features/GuardedCasts.java`).
- **The casts that lowering inserts do not fail**: javac's conversion of an erased type
  variable's value (`Entry<K, V> e = i.next()`) is a `Cast` at its value's position, which the
  analysis does not count as a `ClassCastException`. A deviation: with heap pollution (a raw or
  unchecked use that puts a value of another class in a generic place) Java's cast fails, and
  the translation's then ends the program instead of reaching a `catch`.
- **Still rejected**: a constructor of an exception class that throws (the exception's object is
  built where it is thrown, and by the `From` impl of step 5, neither of which can fail), a
  constructor of an enum that throws (its constants are built where the enum is initialized,
  outside any `try`), and a `toString` that throws something a caller catches (`Display::fmt`
  cannot carry the exception). An instance field initializer that throws is step 8.

### Step 7: try-with-resources

- **Lowered to `try` and `finally`**: `try (R r = new R(..)) body` is `{ R r = new R(..);
  boolean r$failed = false; try { try body catch (Throwable r$thrown) { r$failed = true;
  try { r.close(); } catch (Throwable r$suppressed) { r$thrown.addSuppressed(r$suppressed); }
  throw r$thrown; } } finally { if (!r$failed) { r.close(); } } }`. When the body throws, a
  `close` that throws too is added to the body's exception as suppressed (`getSuppressed()`
  returns it), and the body's exception is rethrown. When the body completes, returns, breaks or
  yields, the `finally` closes the resource, and a `close` that throws propagates. One nested
  `try` per resource closes them in the reverse order, and a resource whose creation fails closes
  the ones before it; the statement's own `catch` clauses and `finally` go around all of that, as
  Java runs them after the resources are closed.
- **An existing variable is the same**: `try (r) body`, and `try (this.r) body` or `try (r)` of a
  final field, is `try (final var r$resource = r) body`, the declared path above with the
  variable's value copied into a hidden local. The value is read when the statement starts, as
  Java reads it; the body cannot change it, since Java requires it to be effectively final. A
  `null` variable is not closed, so it throws no `NullPointerException`: the `close` is guarded
  by `if (r$resource != null)`. Two resources
  of one statement mix freely (a declared one and an existing one), and the close order is the
  same.
- **So only those are accepted**: a resource declared with a `new` object of the program's
  class, or an existing variable of such a class, whose `close()` is the program's (declared in
  the class or a superclass).
  `AutoCloseable` and `Closeable` are accepted in `implements` and left out, as `Comparable` is,
  since the program calls `close` directly. `Throwable.addSuppressed` is the translator's own
  (`crates/translator/java/Standard.java`); its two checks are step 8.
- **Still rejected**: an existing variable of a library type such as `AutoCloseable`, which the
  translator does not know as a type yet (a lambda of it is rejected the same way), and a
  resource of a library class (the library's streams and scanners do not translate). The survey's
  "try-with-resources" feature still counts every such statement, since
  the library's resources are what the survey's sources use.

### Step 8: the leftovers

- **Instance field initializers and constructors that throw**: Java runs an instance field
  initializer in each constructor that calls `super(...)`, outside every `try` of that
  constructor, so what it raises escapes the constructor. `analysis/exceptions/field_inits.rs`
  finds the classes whose field initializers raise something a caller catches; their
  `field_inits` returns a `Result`, and each constructor that runs it passes the failure on as
  `new` does. A `this(...)` constructor runs none of them, so it passes on only what its target
  raises.
- **Static initializer failures**: a `catch` of `Error`, `Throwable`, `LinkageError`,
  `ExceptionInInitializerError` or `NoClassDefFoundError` makes `Standard::select` declare
  `LinkageError`, `ExceptionInInitializerError` (with `(Throwable)` and `(String)` constructors)
  and `NoClassDefFoundError` (with `(String, Throwable)`). `lower/clinit_errors.rs` then rewrites
  the `<clinit>` of each class with a static initializer to this, with the two fields added to
  the class:

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

  The state is the guard, so the rewritten `<clinit>` gets no `init_guard` (section 2.1). Its
  callers return `Result` (`analysis/exceptions.rs` treats a rewritten `<clinit>` as throwing),
  and so does a method that starts by running it (`analysis/exceptions/entry.rs`). Only programs
  that declare these classes are rewritten; the others keep the uncaught translation.
- **`addSuppressed`**: `lower/suppress.rs` puts Java's two checks at the start of the method
  when the program has it: `IllegalArgumentException("Self-suppression not permitted")` for
  itself and `NullPointerException("Cannot suppress a null exception.")` for `null`.
- **Null strings in number parsing**: `Integer`, `Long`, `Short` and `Byte` `parse...` and
  `valueOf` of `null` throw `NumberFormatException` with `Cannot parse null string`, as the JDK
  that is installed here (26) prints it. The brief's `: null` suffix is not in that text, so this
  follows the installed JDK. A null string given to `parseInt(s, begin, end, radix)` or
  `parseLong` is the `NullPointerException` of the runtime's `try_parse_range`; where a `catch`
  sees it, the analysis records it as a dereference (`Facts::null_checked`) and the generator
  writes the checked form (`codegen/parse_range.rs`).
- **Threads**: in a program that starts a thread, the rewritten `<clinit>`'s two fields are
  process-wide statics like the others (`java::Atomic<i32>` and `java::Locked<Option<String>>`).
  Corpus: `threads/StaticInitializerErrorAcrossThreads.java`.
- **Deviations**:
  - Java gives the `IllegalArgumentException` for a self-suppression the exception itself as its
    cause; the translation leaves the cause `null`.
  - A subclass's static initializer whose superclass's initializer failed: the superclass's
    initialization runs in the prologue, before the subclass's state check, and its failure does
    not set the subclass's state. A later use of the subclass therefore throws the superclass's
    `NoClassDefFoundError`, where Java throws the subclass's.
  - The analysis takes the `Error` clause of a rewritten `<clinit>` to rethrow any `Error`, so a
    method may return a `Result` where it need not. An uncaught static initializer that is not
    rewritten also gets an entry point, which may do the same for its callers.
