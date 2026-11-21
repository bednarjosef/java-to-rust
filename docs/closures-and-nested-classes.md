# Closures and nested classes: captures, inner, local and anonymous classes, lambdas, method references, for-each over `Iterable`

How the translator handles code that Java nests inside other code: inner classes, local and
anonymous classes, lambdas and method references, and the enhanced `for` over an `Iterable`.

It builds on three other designs:

- the object model ([object-model.md](object-model.md)): a concrete class is a struct, a type
  that objects of several classes can be is an enum named after it with a variant per class,
  dispatch is a `match`, generics are erased with checked casts at the use sites (real Rust
  generics where proven), and an override with another erased signature gets a bridge method;
- exceptions ([exceptions.md](exceptions.md)): a method returns `Result` exactly where a caller
  observes what escapes it, and the methods of an override group agree;
- multi-threading ([multi-threading.md](multi-threading.md)): a class that threads may share has
  `java::SharedField`s, and a closure handed to a thread must be `Send`.

The guiding rules: generated code is safe Rust that prints what Java prints; what cannot be
proven keeps a correct general representation; lambdas become Rust closures, not hand-made
structs, wherever that is correct.

## 0. The decisions in one page

1. **Every lambda, method reference, anonymous class and local class is a class of the
   program**, desugared as javac desugars it: a synthetic concrete class that implements (or
   extends) its target type, with each captured local as a field and the enclosing instance as a
   field `outer`. So the whole object model and every analysis see them with no special case:
   they get enum variants where they flow, their methods join override groups, nullness and
   exceptions treat a lambda body as an override of the interface method, reachability follows
   their code, and the thread-escape analysis reads their captures as fields. Whether a lambda's
   class is then written as a struct or as a Rust closure is a fact like any other (4.2).
2. **Captures are copies, as in Java.** A captured local is effectively final, so the object or
   closure holds its value at creation: primitives are copied, strings and handles cloned. A
   captured array is a second name for it, so it stays a shared `java::Array`.
3. **An inner class holds its enclosing object in a field `outer`**, set by every constructor
   from the explicit (`ring.new RingIterator()`) or implicit (`new RingIterator()` inside `Ring`)
   enclosing instance. `Outer.this.f` and an unqualified outer member read through it. An
   explicit enclosing instance is checked for null before the arguments are evaluated, as javac's
   `Objects.requireNonNull` does (a `NullPointerException` without a message), so `outer` is
   never null.
4. **A functional interface is a closure type when only lambdas flow into it** and its values
   are only called, stored and passed on. Its values are then `Rc<dyn Fn(A) -> R>` (`Arc<dyn
   Fn(A) -> R + Send + Sync>` when threads may share them). Otherwise the interface is an enum as
   in the object model, with each lambda a variant holding its object. Both forms are correct;
   the closure form is the plainer one.
5. **Default methods of a closure-typed interface are an extension trait** on the closure type,
   translated from the JDK's (or the program's) source, so `cmp.then_comparing(other)` reads as
   in Java.
6. **Method references are lambdas** whose body is the one call. A bound reference evaluates its
   receiver once, where the reference is created.
7. **Exceptions inside lambdas follow the exceptions design unchanged**: the lambda's method is
   in the interface method's override group, so the group returns `Result<R, Throwable>` exactly
   when some caller observes what escapes any lambda of it.
8. **for-each over an `Iterable` is Java's loop written out**: `iterator()`, then `hasNext()` and
   `next()` on each step, calling the translated methods, so an iterator throws what Java's
   throws.

## 1. Captures

Shared by inner, local and anonymous classes and by lambdas.

### 1.1 What is captured and how it is found

javac's tree carries no capture list, so `lower/captures.rs` computes one on that tree before
anything is lowered (`plan_captures`):

- a local variable, parameter or pattern variable that the class's code reads but does not
  declare is captured, found with a stack of the scopes of the locals the code declares;
- a local class created or declared by the code needs its captures there: those that are locals
  of the creating code are passed from them, the others are captured by the creating class too
  (a capture of a capture, as javac does), and a local class extending another passes it the
  superclass's captures;
- the class needs its enclosing instance when its code uses a member of an enclosing class (a
  field or method named without `this`, or `Outer.this`), creates an object of an inner or local
  class whose enclosing instance it must pass, or has a nested class that reaches an instance
  further out through it.

These depend on each other between classes, so they are computed to a fixpoint.

Lowering (`lower/inner.rs`) then desugars each class as javac does, so there are no new
expression kinds: a field `outer` (first in the struct) and a field per captured local; every
constructor takes the enclosing instance first and the captured locals last, and stores them
before anything else runs; every `new`, `this(..)` and `super(..)` passes them. A captured local
is read as its field, and a member of an enclosing class as a chain of `outer` fields
(`this.outer.outer`), except that a constructor's own code reads the parameters (`outer.outer`),
as javac's does; the initializers that a constructor runs read the fields, as javac's do too.
The analyses and the generator see only fields, parameters and arguments they already handle.

In the AST (`ast.rs`), a `TypeDecl` has `outer: Option<usize>` (the enclosing class whose
instance its objects hold) and `captures: Vec<Capture>`, where a `Capture` names the local, the
field that holds it (with `_` appended where the class or a superclass already has a field of
that name) and its type. `Origin` says what the class is: `Member`, `Local`, `Anonymous` or
`Lambda`, the last three with the method that declares it and the binary name that `getName`
and the default `toString` print (`Main$1Labeler`, `Main$1`, `Main$$Lambda`). `JavaTree` names
local and anonymous classes by their binary names in types and owners, since javac's own text
(`<anonymous Runnable>`) is ambiguous between two methods.

### 1.2 How each capture is held

| Captured | Held as |
| --- | --- |
| primitive, Java enum | a field of that type, copied |
| `String` | a `java::JString` field, cloned |
| object, enum value, `Option<..>` | the handle, cloned (`Arc::clone`) |
| object that threads may share | the `Arc`, cloned; a closure holding it is then `Send` |
| array | the array is shared (`java::Array<T>`): the capture is a second name for it |
| `this` | the field `outer`, of the enclosing class's representation |
| `Outer.this` two levels up | through `outer.outer`; no field of its own |
| a capture of an enclosing local or anonymous class | that class's field, cloned again |

For a lambda in closure form, the fields become locals bound before the closure and moved into
it (4.3).

### 1.3 `this` as a value

Capturing `this` uses `this` as a value, which the object model already tracks: a method that
creates an inner, local or anonymous object reading the enclosing object's fields passes `this`,
so the method takes the handle form (`fn iterator(this: &Arc<RefCell<Names>>)`) rather than
`&self`, and the enclosing class is never an owned plain value.

A call of a functional value runs every lambda of the interface's group, so their captures
count when the borrowing analysis asks whether a method may reach an object of its own class
another way. This keeps a callback that calls back into the object running it (a listener that
cancels its own subscription) a re-entrant Java call rather than a `RefCell` panic.

### 1.4 Captures and threads

With captures as fields of a synthetic class, the thread-escape analysis (`analysis/threads.rs`)
needs nothing special: `Thread` stores its `Runnable` in a field that threads share, so the
lambda's class is shared and so are the classes of its fields. rustc then checks `Send`: an `Rc`
or `RefCell` left behind is a compile error, never a miscompilation. A closure that stays on one
thread keeps `Rc`.

### 1.5 Captures in constructors and initializers

`this` captured in a constructor or a field initializer (`new Inner()`, a lambda, an anonymous
class) works: a constructor that uses `this` as a value builds the object behind its handle
first and runs the rest of its body on the handle. Field initializers of a class with an
enclosing instance or captures run inside its constructors, after the synthetic fields are set,
in the order they are written among the initializer blocks, as javac orders them.

The synthetic fields are set after `super(..)`, not before it as javac does, because the
superclass's constructor builds the struct's `base`. A superclass constructor that calls a
method overridden in the nested class, which reads a capture, would see it unset. Such a class
with subclasses is rejected (2.4).

## 2. Inner (non-static nested) classes

### 2.1 Representation

```java
class Names implements Iterable<String> {
    private final String[] items;
    private int size;
    public Iterator<String> iterator() { return new Cursor(); }

    class Cursor implements Iterator<String> {
        private int next;
        public boolean hasNext() { return next < size; }
        public String next() { return items[next++]; }
    }
}
```

The shape of the Rust (details depend on the other analyses):

```rust
struct NamesCursor {
    outer: Arc<RefCell<Names>>,
    next: i32,
}

fn Names__iterator(this: &Arc<RefCell<Names>>) -> Arc<RefCell<NamesCursor>> {
    Arc::new(RefCell::new(NamesCursor__new(this.clone())))
}

impl NamesCursor {
    fn has_next(&self) -> bool { self.next < self.outer.borrow().size }
}
```

- The enclosing instance is the first constructor parameter and the field `outer` (javac's
  `this$0`).
- `iterator()` returns the concrete class where it is the only one under `Iterator`, and the
  `Iterator` enum otherwise, as the object model decides for any type.
- The outer's private fields are read through `outer` directly: the inner class lives in the
  outer's module.
- An inner class is shared, immutable or owned by the usual per-class decision, but `outer` is a
  handle, so in practice it is rarely a plain value.
- An inner object kept in a field of its outer object is a cycle of objects (the outer holds the
  inner, which holds `outer`), which the collector frees like any other.

### 2.2 Construction

| Java | Rust |
| --- | --- |
| `new Cursor()` in an instance method of `Names` | `NamesCursor__new(this.clone())` (the handle form, 1.3) |
| `names.new Cursor()` | `NamesCursor__new(names.clone())`, after a null check of `names` |
| `new Cursor()` in another inner class of `Names` | `NamesCursor__new(self.outer.clone())` |
| `new Cursor()` in a lambda in `Names` | the lambda captures `this` and passes it |

### 2.3 Access to the outer

`Outer.this` and an unqualified name that javac resolves to a member of an enclosing class are
a chain of `outer` reads from the current class to that class (`self.outer`,
`java::fixed!(self.outer, outer)`), each in the outer's form (`borrow()` for `Arc<RefCell<..>>`,
nothing for `Arc<C>`, a lock for a shared one), except that `outer`, which only the constructor
writes, is read with `java::fixed!` (docs/object-model.md, 1.8). javac resolves shadowing: `name` is `self.name`,
`Ring.this.name` is `self.outer.borrow().name`. `Outer.super.m()` calls the superclass's method
on the enclosing object (`super_receiver` in `lower/hierarchy.rs`).

### 2.4 Inner classes in class trees

- **Inner extends inner of the same outer** (`ListItr extends Itr`): `outer` lives in the fields
  struct of the first inner class; the subclass reaches it through `base`, and its constructor
  passes the outer on through `super(..)`, as javac does.
- **A class nested in another class than its superclass** holds its own enclosing instance in a
  field of its own name: `outer` stays the superclass's (in `base`), and the subclass's is
  `outer_` (`outer__` a level further), since the generator reads fields by name
  (`outer_field` in `lower/inner.rs`). This covers an inner class of an inner class extending the
  outermost one, an inner class of a subclass extending the superclass's inner class, local and
  anonymous classes in an inner class's methods, and an inner class of an unrelated class
  (`c.super(..)`). A plain `super(..)` passes the enclosing instance reached through the
  subclass's own `outer`, converted to the outer class's type.
- **`o.super(..)`** passes `o` as the superclass's enclosing instance (`super_init` in
  `lower/hierarchy.rs`). A class whose constructors all give the superclass its enclosing
  instance needs none of its own for it; an inner class of the same outer keeps its own `outer_`,
  since the two objects may differ. A qualified `super(..)` of a superclass that never uses its
  enclosing instance drops the qualifier when it has no effects, and is rejected when it has
  effects.
- **`o.new Inner() { .. }`**: javac's constructor of the anonymous class takes `o` as its first
  declared parameter and calls `o.super(..)`; `new` passes `o` there, before the declared
  arguments (`anonymous_enclosing`). The anonymous class holds an enclosing instance of its own
  only when its code uses one.
- A superclass is found by its erasure wherever javac writes type arguments (javac names an
  inner class of a generic class `java.util.AbstractList<E>.Itr`); `captures::raw` drops every
  type argument list.
- Inner classes implementing interfaces or extending abstract classes (iterators, views such as
  `KeySet extends AbstractSet`) are variants of those types like any class.
- `has_subclasses` in `analysis/hierarchy.rs` holds for every class with subclasses;
  `fields_structs` picks those that get a fields struct.

Rejected:

- a class with subclasses whose constructor uses `this` as a value or calls an overridable
  method (the whole-object construction of `analysis/whole_objects.rs`, whose zero object is
  built with the enclosing instance and captured locals) when a `super(..)` computes them
  other than as its constructor's parameters, fields of them, or either upcast (an inner class
  of a subclass passes its own enclosing instance to a superclass that is an inner class of the
  enclosing class's superclass);
- an enclosing instance needed in static code (javac rejects it too);
- `Outer.this` or `Outer.super` from an inner class that itself extends `Outer`: the search for
  the enclosing instance stops at the first subclass of `Outer`, which is then `this`.

### 2.5 Naming

Nested classes are named by concatenation (`Bank.InsufficientFunds` is `BankInsufficientFunds`),
in the module of their package:

- inner classes: `NamesCursor`, `HashMapKeyIterator`;
- local classes: `<Outer><Name>` (`MainLabeler`), with the method appended only where two local
  classes of one class share a name (`MainBoxInSecond`);
- anonymous classes: `<Supertype>In<Place>` (`ComparatorInMain`, `TaskInInit` in a field
  initializer), numbered from 2 on a clash;
- lambda classes: `<Interface>LambdaIn<Method>` (`IntOpLambdaInMain`, `IntOpLambdaInMain2` in
  source order), in the package of the enclosing top-level class;
- member classes of local and anonymous classes: after their outer class and their own name
  (`Main$1Counter.Tally`), capturing what they read like local classes (`member_outers`).

`getName` gives javac's binary name (`Main$1Labeler`, `Main$1`); `getSimpleName` of an anonymous
class is `""`. A lambda's class name is `Main$$Lambda` (`p.Outer$Inner$$Lambda` in a member
class), the JVM's name without its varying suffix.

### 2.6 Generic outers

Inner classes of generic classes translate erased, like their outer
(`corpus/features/GenericInnerClassOfGenericClass.java`,
`InnerSubclassesOfGenericClasses.java`). Not done: a Rust generic inner class that repeats the
outer's type parameters (`struct RingRingIterator<T> { outer: Arc<RefCell<Ring<T>>>, .. }`).

## 3. Local and anonymous classes

### 3.1 Local classes

```java
String prefix = "item-";
class Labeler { String label(int n) { return prefix + n * n; } }
Labeler labeler = new Labeler();
```

```rust
struct MainLabeler { prefix: String }
let labeler = MainLabeler__new(prefix.clone());
```

The captures are fields and trailing constructor parameters (javac's `val$prefix`); `new
Labeler()` passes them from the scope. A local class that uses the enclosing instance has
`outer` too, first. Local classes may extend classes, implement interfaces and be extended by
other local classes of the same block; they are concrete classes like any other.

Local records, enums and interfaces are implicitly static: they capture nothing and move to the
top level (`javac/records.rs`, `lower/local_types.rs`), named by package and simple name, with
the method appended on a clash (`app.PointInRun`). One declared in an initializer block is not
moved, and is rejected.

Rejected: creating a local class at a place where a local hides one of its captures.

### 3.2 Anonymous classes

An anonymous class is a synthetic concrete class whose supertype is the named type. Its one
constructor takes `outer` (in an instance context), the captures and the arguments of the
`new`, which it passes to `super(..)`: `new LinkedHashMap<>(16, 0.75f, true) { protected boolean
removeEldestEntry(..) { .. } }` is a subclass with a `super(16, 0.75, true)` constructor and an
override that the object model dispatches to.

- Field initializers and instance initializer blocks run in its constructor as for any class.
- `this` is the anonymous object; the enclosing instance is reached through `outer`.
- It is a concrete class of the hierarchy, so it gets a variant of every type it flows into and
  `instanceof` and casts see it. The program never names it, so it is only reached through its
  supertype.
- One that overrides a method called from its superclass's constructor gets the object model's
  whole-object construction (object-model.md, 1.4), so the superclass's constructor calls the
  override.
- One that extends or implements a JDK type (`new Runnable() { .. }`) needs that type as a
  program type: the translator's declarations of the functional interfaces in a single file
  (4.6), the JDK's sources in a whole program.
- Anonymous classes are never written as closures, even of a functional interface: an interface
  that one implements is an enum.

### 3.3 Enum constant bodies

A constant with a body (`PLUS { int apply(int a, int b) { .. } }`) is an anonymous subclass of the
enum; since an enum is a Rust `enum`, `lower/constant_bodies.rs` turns its overrides into arms of
a `match` on `self` in the enum's method. A body may override the enum's methods, or the
methods of an interface the enum leaves to its constants; a method that overrides nothing is a
method of the enum, which only that body's code calls. A body's `super.m(..)` runs the enum's
own `m`, which the `match` no longer is for an overriding constant, so the enum's own code is
also a method of its own, `m$super` (`m_super` in Rust), which those calls run, as Guava's
`LocalCache.EntryFactory` copies an entry (`corpus/whole-program/EnumConstantSuperCalls.java`).
An override is matched by its name and
parameters, so one that narrows the result type (`Double readNumber` of Gson's `ToNumberPolicy`
beside the others' `Number readNumber`) is an arm of the same method, which returns the enum's
type, or the interface's where the bodies narrow it differently
(`corpus/features/CovariantConstantBodies.java`). Lambdas and method references in
those methods and in the initializers of the body's fields translate and capture the constant as
`this` (`corpus/whole-program/LambdasInConstantBodies.java`, `ConstantBodyFieldLambdas.java`). A
constant with a body calls the enum's constructor through the body's anonymous one, which javac
does not mark as variable-arity, so its varargs are packed by the enum's own constructor.

A body may declare fields of type `int`, `long`, `double`, `float`, `char`, `boolean` or
`String`: each becomes a field of the enum whose value for a constant is that body's
initializer, or the zero of its type for the others. Where the enum's fields are recomputed on
each read the initializer must be a constant; where they are tables (an enum whose constructor
runs code, `lower/enum_tables.rs`) it runs right after the constant's constructor, with the
constant as `this`. Rejected in a body: blocks, member classes, static fields, fields of other
types, initializers that are not constants in an enum without tables, methods that override a
method of `Object` that the enum does not declare, and methods that override nothing of one
name and signature in two bodies.

## 4. Lambdas, method references and functional interfaces

### 4.1 Lowering: a lambda is a class

`JavaTree` prints, with each `LAMBDA_EXPRESSION` and `MEMBER_REFERENCE` node, a `descriptor`: the
interface's one abstract method that is not `Object`'s, with the interface declaring it, the
target interface, its declared and erased types, and its type after javac's substitution
(`substituted`).

`lower/lambdas.rs` declares a synthetic class per lambda (`NestKind::Lambda`, `Origin::Lambda`)
before anything is lowered, among the members of the class whose code holds it (an enclosing
lambda's class, for a lambda in a lambda), so `lower/captures.rs` finds its captures as for a
local class. The lambda expression lowers to `new` of that class converted to the interface,
`Cast(Interface, New { class, args, .. })`; the arguments of the `new` are the captures. So the
object model's variants, dispatch and bridges take it with no new case.

The class's one method takes javac's substituted types: a lambda of `Function<Integer, Integer>`
is `apply(Integer)` returning `Integer`, with the bridge `apply(Object)` that any class
implementing the generic interface gets. A generic method of the interface, which only a method
reference implements (`<A> Palette<A> create(int, List<A>)` of the Minecraft server's
`Palette.Factory`, made by `SingleValuePalette::create`), has its own type variables erased in
`substituted`, since the lambda's class has nowhere to declare them.

A method reference's body is one call, built as javac tree JSON (`METHOD_INVOCATION`,
`NEW_CLASS` or `NEW_ARRAY` on parameters `a`, `b`, ..) and lowered like a written call, so it
gets the same conversions and library mappings and javac's referent symbol picks the overload:

| Kind | Body |
| --- | --- |
| static `Util::twice` | `Util.twice(a, b)` |
| unbound `String::length` | the first parameter is the receiver: `a.length()` |
| bound `prefix::concat`, `System.out::println`, `this::helper` | the receiver is a capture: `receiver.concat(a)` |
| constructor `ArrayList::new` | `new ArrayList<>(a)`, the overload javac resolved |
| array constructor `int[]::new` | `new int[a]` |

A bound receiver other than `this` (a local, a field, `a.b()`) is evaluated once where the
reference is created, into a synthetic local of the creating code that is checked for null
there, as javac's `Objects.requireNonNull` does (its exception has no message), and that the
class captures (`plan_receivers`). Without the JDK's sources the program has no `PrintStream`
class for that local, so `System.out` and `System.err` are read where the reference is called
(a `System.setOut` between creating and calling it is not translated then anyway).

`super.m()` and `super::m` in a lambda run on the object of the class whose code holds the
lambda, as javac's synthetic method does. Any other use of `super` in a lambda (including
`super.clone()`) is rejected.

Marker interfaces (`Serializable`, `Cloneable`, `RandomAccess`) declare nothing: without the
JDK's sources, which declare them, lowering drops them where a class implements them
(`lower/functional.rs`); with those sources they are interfaces like any other, so that a
`ZoneRules` returned as a `Serializable` is one. `JavaTree` drops them and
`Object` from the type of a lambda or method reference cast to an intersection, so
`(Comparator<T> & Serializable)` is `Comparator<T>`. A lambda cast to two interfaces that are not
markers (`(Runnable & Tagged)`, with an empty `Tagged` of the program's) is rejected: the closure
would not be a `Tagged`. Other intersection casts are in the object model's generics.

### 4.2 Which functional interfaces are closure types

`analysis/closures.rs` computes `Facts::closure_types`, the interfaces whose values are closures.
An interface is one when all of these hold:

1. It is functional: it declares one abstract method besides `Object`'s (which `Comparator`
   declares again), or extends exactly one interface and declares none (`UnaryOperator<T>
   extends Function<T, T>`). `Object` and `Comparable` never are.
2. Every class below it is a lambda's class that implements just this one interface, and at
   least one exists. A named class or an anonymous class implementing it makes it an enum.
3. Every interface above and below it is a closure type too: they share its closures.
4. Every `new` of its lambda classes is converted to the interface right away, and nothing does
   with a value of it anything but call its method or its default methods, store it and pass it
   on: no conversion to another type than an interface above it (to `Object`, for printing, into
   a collection), no `==`, no `instanceof`, no call of a builtin with it.
5. Each lambda's method uses `this` only to read its capture fields.

Whatever rules out one interface rules out those above and below it. Everything else keeps the
object model's enum form, one variant per lambda class. The decision is per interface and
program-wide; both forms coexist in one program.

### 4.3 Forms of a closure-typed value

The interface is a type alias (`codegen/closures.rs`):

```rust
type IntOp = Rc<dyn Fn(i32, i32) -> i32>;
type UnaryOperator = Function;   // an interface that extends another names its closures
```

so a `UnaryOperator` is passed where a `Function` is taken. The argument and result types are
those of the interface's method as its dispatcher would take them (`Fn(&java::Wtf8) -> i32` for a
borrowed string parameter, `-> Result<_, Throwable>` when the method throws).

- **Stored form** (fields, statics, array elements, returns, and the parameters that are not
  lent): `Rc::new(closure) as IntOp`, so every place gets the alias's type without an
  annotation. Such a parameter is `&IntOp`, as the borrowing analysis lends objects.
- **Lent form**: a parameter that the method only calls and passes on to other lent parameters
  (never assigns, stores, captures in another lambda, returns or uses with a default method;
  `Facts::lent_closures`) is `&dyn Fn(i32, i32) -> i32`, or `Option<&dyn Fn(..)>` where it may be
  null, with no `Send + Sync` even for a closure type that threads share, since the closure runs
  on the thread that lends it. The methods of an override group take it alike, so it is lent in
  all of them or in none; a constructor's parameters and a lambda's own are never lent. A lambda
  passed to it is the closure itself, made on the caller's stack and lent,
  `each(n, &(|v: i32| { .. }))`, so evaluating it allocates nothing and drops no `Rc`; it
  borrows its captures as a local closure does. A lent parameter or a local closure is passed as
  it is, `null` as `None`, and a stored closure as a borrow of what its handle holds
  (`&**(&op)`, `(&op).as_deref()`), so both forms meet at one call.
- **Local form**: a local whose every declaration has a lambda as its initializer and that is
  only called and passed to lent parameters (never assigned, captured, otherwise passed,
  returned or used with a default method; `Facts::closure_locals`) is the closure itself,
  `let f = |x: i32| -> i32 { .. };`, lent as `&f`. It borrows what it captures when each capture
  is a local or a clone of one (`this` included), and moves its captures in otherwise.

The closure is written where the lambda is created: the lambda class's method is generated
inside the creating expression with its own names and facts, and its capture fields become
locals bound before the closure and moved in (`field_locals`), so `this.k` is `k` and
`this.outer` is `outer`:

```rust
{ let k = k; Rc::new(move |x: i32| -> i32 { x + k }) as IntOp }
```

A closure with no captures has no `move`. Where the closure is a function's final value, the
bindings are statements of the function and the result type converts `Rc::new(..)` without
`as`.

### 4.4 Generics and bridges

A generic interface's closure is erased, per interface: `Function<Integer, Integer>` is `type
Function = Rc<dyn Fn(Object) -> Object>;`, and a call boxes the arguments and casts the result
as the enum's dispatcher would. The lambda's method, typed by javac with the substituted types,
sits behind its bridge, so the closure's body is the bridge's casts of the arguments, then the
method's body, then the bridge's conversion of its value:

```rust
Rc::new(|t: Object| -> Object {
    let x: i32 = java::cast(t.as_integer(), "java.lang.Integer");
    Object::Integer(x.wrapping_mul(x))
}) as Function
```

A result that needs no conversion (`Predicate.test`, `Consumer.accept`) takes the method's body
as the closure's whole body. A method that returns early or throws, and whose result is
converted, is a closure called at once inside the outer one, since its `return`s would otherwise
leave the outer closure with the unconverted value.

### 4.5 Calls

A call of the abstract method on a closure-typed value calls the closure: `f.apply(x)` is
`f(x)`, `r.run()` is `r()` or `r()?`. A closure read from a field of a shared object is cloned
out of the `RefCell` before it runs, `({ let f = a.borrow().op.clone(); f })(3)`, since its body
may reach the same object. A local, a capture, a field of a plain object or a field that only
constructors write (docs/object-model.md, 1.8) is called in place (`op(x)`, `(self.op)(x)`,
`(java::fixed!(a, op))(x)`). Any other closure that may be null, such as an element of an
array (an enum's field kept as a table of the constants' values) or a method's return value, is
dereferenced first, `(java::non_null(f()))(x)`, so that null throws `NullPointerException`
there, in the checked form where a `catch` may see it. On an enum-typed interface the call is
the dispatcher, as for any interface.

### 4.6 Default and static methods; the JDK's functional interfaces

Default methods of a closure type are an extension trait beside its alias, with only the
default methods the program calls:

```rust
trait PredicateMethods { fn negate(&self) -> Predicate; .. }
impl PredicateMethods for Predicate { .. }
```

`self` is the closure: `test(x)` is `self(x)`, and a lambda that captures `this` gets `let outer
= self.clone();`. A call is method syntax (`empty.negate().and(longer)`), and a module that calls
another module's default methods imports its trait
(`use crate::java_::util::function::PredicateMethods;`). Two closure types with the same Rust
type and a default method of the same name would make such a call ambiguous; none of
`java.util.function` do. Static methods are functions like any other static method.

**With the JDK's sources** (a whole program), the JDK's functional interfaces are translated
from the JDK like the program's own, with their default and static methods and the lambdas in
them (`Comparator.comparing`, `thenComparing`, `reversed`, `nullsFirst`, `Function.andThen`,
`Predicate.negate`, ...). The methods of `java.util.Objects` other than the hand-written
`equals`, `hashCode`, `hash`, `isNull` and `nonNull` are the translated `Objects.java`.

**In a single file**, `Runnable`, `Callable`, `Comparator`, `Iterable`, `Iterator` and the
`java.util.function` interfaces come from `crates/translator/java/Functional.java` (compiled by
`build.rs`), which declares only their abstract methods; a program gets the ones it names, as
interfaces of the program (`lower/functional.rs`). Their default and static methods are not
there, since a hand-written copy would duplicate the JDK's code: a call of one is rejected with
a message that points to whole-program mode (`--main` and `--library`). A lambda of another JDK
interface is rejected in a single file.

### 4.7 Exceptions in lambdas

The lambda's method is in the override group of the descriptor, so `analysis/exceptions.rs`
applies as written: the group returns `Result<R, Throwable>` exactly when some caller of the
interface method observes what escapes it. The closure type is then `Rc<dyn Fn(A) ->
Result<R, Throwable>>` and calls are `f(a)?`. A thread's entry observes everything, so a
`Runnable` given to a `Thread` returns `Result` whenever its body can throw. A user interface
with `throws` is the same case.

A consequence: the signatures of a group are unified, so one lambda that may return null makes
every lambda of the interface return an `Option`, and one that throws makes every call `?`. This
is the object model's rule for overrides.

### 4.8 Recursion and identity

A lambda cannot capture itself through a local (not effectively final). Through a field it
reads the field at call time, so a static `FIB = n -> .. FIB.apply(n - 1) ..` reads the static
on each call, and an instance field set to a lambda capturing `this` makes a cycle through the
closure's `Rc`, which is never freed.

Closure-typed values are never compared (4.2, condition 4). A lambda in enum form is an object like
any other: compared by identity, and printed as `Main$$Lambda@<hash>`. A lambda or method reference
that captures nothing is one object per place it is written, as HotSpot makes it (the JLS leaves
this open; `captures_nothing` in `codegen/objects.rs`), which also saves an allocation each time it
runs. Where the object is its struct alone, or one in a `java::Monitored`, it is a constant in a
`static` at that place (`java::StaticObject`), outside the heap, and taking it is loading the
static's address: nothing makes it at run time, and no code checks whether it was made. 4-byte
references (`JAVA_TO_RUST_COMPACT_REFERENCES`) name only objects in the heap, so under them, as
where the struct is in a `RefCell` or a `java::Locked`, it is not a constant. Otherwise it
is made the first time the place runs and kept in a `static` (`java::PlaceObject`), shared by every
thread, as an object without fields can be unless it is a `RefCell`, whose borrow flag is not
atomic: in a program that starts threads, such a place keeps a thread-local `OnceCell` instead, one
object per thread. A thread-local takes its room in every thread's static thread-local storage,
which the C library zeroes for each thread it starts and the collector scans at each collection: the
Minecraft server's 14,300 places, all thread-local, took 114 KB of each of its 60 threads' 120 KB. A
lambda that captures something is a new object at each evaluation.

### 4.9 Threads

A closure type that threads may share (`Facts::thread_shared` holds it or an interface above or
below it) is `Arc<dyn Fn(..) -> R + Send + Sync>`, its closures `Arc::new(..)`, the handle chosen
in one place (`closure_handle` in `codegen/closures.rs`); what they capture is of shared types by
the same walk. Other closure types keep `Rc`, also in a program with threads. Corpus:
`corpus/threads/ClosuresAcrossThreads.java`, `ClosureMethodsAcrossThreads.java`,
`OperatorsAcrossThreads.java`.

## 5. for-each over `Iterable`

Java's enhanced `for` over a non-array is `for (Iterator<T> it = c.iterator(); it.hasNext(); ) {
T x = (T) it.next(); body }`. `lower/foreach.rs` lowers it to exactly that:

```text
{ Iterator it = c.iterator(); while (it.hasNext()) { T x = (T) it.next(); body } }
```

with ordinary calls of the program's (or the JDK's) methods, resolved as javac would. The
generator writes `while it.has_next() { let x = it.next(); .. }`, propagating what the calls
throw like any call. Since `hasNext` and `next` are translated code, the loop does what Java's
does, including `ConcurrentModificationException`, `NoSuchElementException`, the quirk that
removing the second-to-last element ends an `ArrayList` loop, and the call sequence after
`break`. A label on the loop goes on the `while`. The collection may be an object or an enum
constant whose enum implements `Iterable`.

- javac's tree does not show the calls, so reachability adds `Iterable.iterator`,
  `Iterator.hasNext` and `Iterator.next` at every for-each over a non-array
  (`program/reach.rs`).
- A collection type without a no-argument `iterator()` is rejected ("for-each over
  collections").
- In a single file, `Iterable` and `Iterator` come from `Functional.java` with their abstract
  methods only, so `Iterator.remove` (a JDK default method) is rejected there;
  `corpus/features/IteratorRemove.java` declares its own iterator for it. With the JDK's sources
  the JDK's `Iterator` is translated, default methods included.

## 6. Not done

Plans, each a refinement of a correct form that exists:

- **Lambdas that capture a lent parameter**: a lambda that captures a closure parameter (the
  server's `getEntities` wraps its consumer in the lambda it passes on) makes that parameter a
  stored one today. Where the capturing lambda is itself lent, it could borrow the lent
  closure, `&dyn Fn` inside `&dyn Fn`, and the outer parameter stay lent.
- **Parameter form `impl Fn(A) -> R`**: a lent parameter as a generic, so that the helper is
  instantiated per lambda and LLVM can inline the closure's body into the helper's loop. A
  method that wraps its function parameter into a new lambda and recurses would instantiate
  itself without end, so it keeps `&dyn Fn`.
- **Closure types per instantiation**: `Function<Integer, Integer>` as `Rc<dyn Fn(i32) -> i32>`
  at a fully parameterized site, with a bridging closure where it meets an erased one, so a
  program's own lambdas stay typed when handed to erased JDK code. Today the closure type is
  erased per interface.
- **Type aliases per signature** rather than per interface.
- **Closure types seen as objects**: identity, `equals`, printing or storing in a collection
  through `Object` (a variant `Object::<Interface>(Rc<dyn Fn ..>)` compared by `Arc::ptr_eq`).
  Today any such use makes the interface an enum.
- **Anonymous classes as closures**, those without fields of their own and created directly in
  a conversion to a functional interface; with fields they would need a `RefCell` inside the
  closure.
- **`Box<dyn Fn>`** for a field that is the value's only holder, from the moves analysis.
- **A method path instead of a closure** where the signatures match (`i32::abs`).
- **An idiomatic `for x in ..` loop** over an iterator whose `hasNext` and `next` do not throw,
  through a runtime adapter that calls `has_next` then `next` on each step; and `impl Iterator`
  for iterator classes that only read their fields and the outer (thorough mode,
  [thorough-mode.md](thorough-mode.md); the `Vec` form of `ArrayList` in
  [lists-and-streams.md](lists-and-streams.md)).
- **Per-site decisions with the points-to analysis** (thorough mode): more `this` captures
  borrowed and more methods kept `&self`; a lambda class `Arc` only at the sites a thread
  reaches; a named class implementing a functional interface and only used through it
  (`ReverseComparator2`) inlined into a closure, so `Comparator` stays a closure type.

## 7. Where it lives

| File | What |
| --- | --- |
| `crates/translator/java/JavaTree.java` | the `descriptor` of lambdas and method references; binary names of local and anonymous classes; marker interfaces dropped from the intersection a lambda is cast to |
| `crates/translator/java/Functional.java` | the JDK's functional interfaces, `Iterable` and `Iterator`, abstract methods only, for single files |
| `lower/captures.rs` | what each nested class captures and which enclosing instances it needs |
| `lower/inner.rs` | declaring, naming and desugaring inner, local and anonymous classes |
| `lower/lambdas.rs` | lambda and method reference classes, their methods and receivers |
| `lower/functional.rs` | functional interfaces for single files, marker interfaces |
| `lower/local_types.rs` | local records, enums and interfaces |
| `lower/constant_bodies.rs` | enum constant bodies |
| `lower/foreach.rs` | for-each over `Iterable` |
| `lower/hierarchy.rs` | `o.super(..)` and `Outer.super.m()` |
| `analysis/closures.rs` | which interfaces are closure types; lent closure parameters; closure locals |
| `codegen/closures.rs` | aliases, closures, calls, extension traits, `Rc` or `Arc` |

Corpus programs: `corpus/features/` (`LocalClasses`, `CapturedLocals`, `LocalMemberClasses`,
`InnerClasses`, `InnerClassTrees`, `InnerIterators`, `InnerSuperclassesWithExplicitOuters`,
`AnonymousClasses`, `AnonymousSubclasses`, `ForEachIterable`, `IteratorRemove`, `LambdaObjects`,
`LambdaMadeOnce`, `LambdaCaptures`, `MethodReferences`, `MethodReferenceReceivers`, `SuperInLambdas`,
`ClosureForms`, `LocalClosures`, `ClosureDefaultMethods`, `GenericClosures`), and in
`corpus/whole-program/` (`FunctionChains`, `Comparators`, `ReverseOrder`,
`LambdasInConstantBodies`, `ConstantBodyFieldLambdas`, `LambdaPlaceObjects`, `LentClosures`),
which run only with the JDK's sources.
