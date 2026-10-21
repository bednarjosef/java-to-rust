# Closures and nested classes: captures, inner, local and anonymous classes, lambdas, method references, for-each over `Iterable`

This plan builds on the object model (`docs/object-model.md`: a concrete class is a struct, a
type with several concrete classes is an enum named after it, variants are the classes whose
objects flow into the type, dispatch is a `match`, generics are erased with `Cast` nodes at the
use sites and real where proven, bridges are methods), on the exceptions design
(`docs/exceptions.md`: a method returns `Result` exactly where a caller observes what escapes it,
override groups agree) and on the multi-threading design (`docs/multi-threading.md`: a class is
thread-shared when a root reaches it, `Arc<java::Locked<C>>`, a closure handed to a thread must
be `Send`). It is written for two implementers working in parallel and is kept up to date as
decisions change (see "Deviations" at the end).

Decided by the user and kept here: class trees are enums (closed world); generics are idiomatic
where proven and erased otherwise; exceptions are `Result`; programs are normally
multi-threaded; hand-written are only natives, core types and direct Rust equivalents, the rest
of java.base is translated; generated code is safe Rust that prints what Java prints, and what
cannot be proven keeps a correct general representation; lambdas become Rust closures, not
hand-made structs, wherever that is correct.

## 0. The decisions in one page

1. **Every lambda, method reference, anonymous class and local class is a class of the
   hierarchy table.** Lowering turns each into a synthetic concrete `TypeDecl` in `Class::types`
   that implements (or extends) its target type, with the captured variables as its fields and
   the enclosing instance as a field `outer`, exactly as javac desugars them. So the whole
   existing machinery sees them for free: `Hierarchy::targets` makes a call of a functional
   interface's method run every lambda converting to it, `flows` gives them enum variants,
   nullness and exceptions unify their bodies with the interface method's override group,
   reachability follows their bodies, and the thread-escape analysis reads their captures as the
   fields of a class. The *representation* of such a class is then a fact like any other: a
   struct (the faithful form, always available), or a Rust closure where that is proven right.
2. **Captures are copies, as in Java.** A captured local is effectively final, so the closure or
   object holds its value at creation: primitives are copied, strings and handles cloned
   (`Rc::clone`, `Arc::clone`), a plain `Vec` array becomes a shared `java::Array` because the
   capture is a second name for it, and the value is moved instead of cloned when the local is
   dead afterwards (`Facts::moves`). `this` is captured as the object's handle, which makes the
   enclosing method use the handle form of `this` (not `&self`) whenever the capture outlives the
   statement; a lambda that provably does not outlive the call it is passed to borrows instead
   (`|x| self.total += x`, Rust 2021 disjoint field capture).
3. **An inner class holds its outer instance in a field `outer`** of the outer's representation
   (`Rc<RefCell<Outer>>`, `Rc<Outer>`, `Arc<..>`), set by every constructor from the explicit
   (`ring.new RingIterator()`) or implicit (`new RingIterator()` inside `Ring`) enclosing
   instance. `Outer.this.f` is `self.outer.borrow().f`; an inner class in a class tree keeps
   `outer` in the fields struct of the first inner class of that outer, so subclasses reach it
   through `base`. Rust names concatenate as for static nested classes (`RingRingIterator`).
4. **A functional interface is a closure type when only lambda-like values flow into it**: every
   value of the type comes from a lambda, a method reference, or an anonymous class created
   directly in a conversion to it, and nothing calls `equals`/`hashCode`/`getClass` on it. Its
   values are `Rc<dyn Fn(A) -> R>` (`Arc<dyn Fn(A) -> R + Send + Sync>` when thread-shared), a
   parameter that is only called is `impl FnMut(A) -> R` with the lambda written inline, and a
   local that never escapes is a plain `let f = |x| ..`. Otherwise (a named class implements it,
   as `Comparators.NullComparator` does `Comparator`, or identity is observed) the interface is
   an enum as in the object model and each lambda is a variant holding its struct. Both forms
   are correct; the closure form is the idiomatic refinement.
5. **Closure types follow the generics decision per value, with bridges.** `Function<Integer,
   Integer>` at a fully parameterized site is `Rc<dyn Fn(i32) -> i32>`; inside erased JDK code
   `Function<? super K, ? extends V>` is `Rc<dyn Fn(Object) -> Object>`; a conversion between
   the two is a wrapping closure, which is javac's bridge. So a program's own lambdas are typed
   even when they are handed to the erased `HashMap.computeIfAbsent`.
6. **Default and static methods of closure-typed interfaces are translated from the JDK's source
   onto the closure type**: `thenComparing`, `reversed`, `andThen`, `negate` become methods of a
   generated extension trait implemented for the `Rc<dyn Fn ..>` type, so
   `cmp.then_comparing(other)` reads as in Java; static methods (`Comparator.comparing`,
   `Function.identity`) are free functions of the interface's module.
7. **Method references are lambdas** lowered to the same synthetic class: static `C::m` is
   `|a| C::m(a)`, bound `obj::m` evaluates the receiver once at creation (Java's null check
   there) and captures it, unbound `C::m` takes the receiver as the first parameter, `C::new`
   builds the object, `int[]::new` the array. Where the bound signature matches, the generator
   may write the path (`i32::abs`) instead of a closure; that is a later refinement.
8. **Exceptions inside lambdas follow the exceptions design unchanged**: the lambda's body is a
   method in the abstract method's override group, so the group returns `Result<R, Throwable>`
   exactly when some caller observes what escapes any lambda, and the closure type is then
   `Rc<dyn Fn(A) -> Result<R, Throwable>>` and calls are `f(a)?`. A user functional interface
   with `throws` is the same case.
9. **for-each over an `Iterable` is Java's protocol**: `let it = c.iterator(); while
   it.has_next() { let x = it.next(); .. }`, where `iterator`, `has_next` and `next` are the
   translated methods (dispatched through the `Iterator` enum when several iterator classes
   exist). When neither method returns `Result`, the loop is `for x in java::iterate(it)`, an
   adapter in the runtime that calls `has_next` then `next` on each step exactly as Java's loop
   does. A Rust `for` directly over the collection (`for x in &v`) belongs to
   `docs/lists-and-streams.md` (the `Vec` representation of `ArrayList`) and to thorough mode.
10. **Identity of functional values is `Rc::ptr_eq`**: `==` on the same variable is true, two
    evaluations of one lambda expression are different objects (the JLS leaves this unspecified;
    HotSpot happens to cache non-capturing lambdas, which no corpus program may depend on).
    `toString` of a lambda prints `Main$$Lambda@<hash>`, as Java's shape; the hash differs as
    every identity hash does.
11. **Rejected for now**, each with a message naming the construct: `this` captured inside a
    constructor or field initializer (the struct-literal constructor has no handle yet), lambdas
    and blocks or classes in an enum constant's body, an intersection cast with two non-marker
    interfaces (`(Runnable & Callable)`; `(Comparator<T> & Serializable)` keeps `Comparator`),
    and inner classes of generic classes under real generics (they stay erased).

## 1. Captures

Shared by inner, local and anonymous classes and by lambdas; written first by the nested-classes
implementer (section 7).

### 1.1 What is captured and how it is found

javac's tree carries no capture list, so lowering computes it. While lowering a method body,
`lower/captures.rs` keeps a scope stack of the locals declared so far (parameters, locals, catch
parameters, pattern bindings, for-each variables). On entering a lambda body, an anonymous class
body or a local class declaration it pushes a *boundary*; every `IDENTIFIER` whose symbol is a
`LOCAL_VARIABLE`, `PARAMETER`, `EXCEPTION_PARAMETER` or `BINDING_VARIABLE` declared on the
far side of the innermost boundary is a capture of that boundary, and of every boundary between
(a capture of a capture: a lambda inside an anonymous class inside a method captures through
the anonymous class, which gets the field too, as javac does). `this`, an unqualified call or
field of the enclosing class, and `Outer.this` are captures of the enclosing instance, one per
enclosing class reached (`uses_outer: Vec<usize>`). A local class is a boundary for its whole
body and its constructor calls (`new Labeler()`) pass the captures implicitly.

```rust
// ast.rs
pub struct TypeDecl {
    // as on branch object-model, plus:
    /// The enclosing class whose instance objects of this class hold in `outer`: an inner class,
    /// or a local, anonymous or lambda class created in an instance context; else `None`.
    pub outer: Option<usize>,
    /// Locals of the enclosing method that the bodies read: fields of the struct, set by every
    /// constructor from the creating expression's scope. Empty for named member classes.
    pub captures: Vec<Capture>,
    pub origin: Origin,
}
pub struct Capture { pub name: String, pub ty: Type }
pub enum Origin {
    Named { local_in: Option<usize> },          // a named class; `Some(method)` for a local class
    Anonymous { method: usize },                 // `new T(args) { .. }`: implements or extends `T`
    Lambda { method: usize, descriptor: usize }, // implements the functional interface's method
}
pub enum ExprKind {
    // as today, plus:
    /// `new C(args)` with the enclosing instance of an inner class: explicit (`o.new C()`) or the
    /// implicit `this`/`Outer.this`. Captures come from `TypeDecl::captures` and the scope.
    New { class: usize, args: Vec<Expr>, ctor: Option<usize>, outer: Option<Box<Expr>> },
    /// `Outer.this`, or the implicit enclosing instance of an unqualified outer member.
    OuterThis { class: usize },
    /// A lambda or method reference: creates an object of synthetic class `class`; `ty` is the
    /// functional interface type it converts to.
    Lambda { class: usize },
}
```

A captured local keeps its Java name inside the body; the generator reads `TypeDecl::captures`
to write the struct's fields or the closure's prelude.

### 1.2 How each capture is held

| Captured | In a struct (inner, local, anonymous, lambda-as-object) | In a closure |
| --- | --- | --- |
| primitive, Java enum | a field of that type, copied | copied into the closure (`move`) |
| `String` | `String` field, cloned, moved when the local is dead (`moves`) | the same, in a prelude block |
| object `Rc<RefCell<C>>`, `Rc<C>`, enum value, `Option<..>` | the handle, cloned (`Rc::clone`) or moved | the same |
| thread-shared object | `Arc<..>`, cloned | the same; the closure is then `Send` |
| plain array `Vec<T>` | the capture is a second name: `analysis/arrays.rs` keeps the array shared (`java::Array<T>`), converted once with `java::share` at the capture if the local was plain so far | the same; or the `Vec` moved into the closure when the local is dead afterwards and the closure is `FnMut` (see 4.3) |
| `this` (the enclosing instance) | field `outer: Rc<RefCell<Outer>>` (the outer's representation) | cloned handle, or borrowed (`&self`/`&mut self`) when the closure does not outlive the call (4.3) |
| `Outer.this` two levels up | reached through `outer.outer`; no field of its own | cloned from `self.outer.borrow().outer` at creation |
| a capture of the enclosing anonymous or local class | that class's field, cloned again | the same |

The closure prelude is the shape the multi-threading design already uses:

```rust
Rc::new({
    let weight = weight;            // omitted: a Copy value needs no prelude line
    let seen = seen.clone();
    let this = this.clone();
    move |x: i32| { .. }
})
```

Only the clones are written; `Copy` values are captured by `move` directly.

### 1.3 `this` as a value

Capturing `this` is using `this` as a value, which the object model already tracks for
conversions ("`this` converted inside its methods" keeps a class from being owned). Two facts
follow:

- **Owned classes** (plain values): a class whose `this` is captured by a stored closure or by
  any inner, local or anonymous object is never owned, because the capture is a second holder.
  A class whose `this` is only captured by borrowed closures (4.3) may stay owned.
- **The handle form**: `analysis/borrowing.rs` lets a method take `&self`/`&mut self` when it
  uses `this` only for fields and such calls, and nothing it runs reaches an object of its class
  another way. A `this` capture that outlives the statement (stored closure, `new Inner()`,
  anonymous object returned or stored) needs the handle, so the method keeps the form
  `fn iterator(this: &Rc<RefCell<Names>>)`; a call of a functional value (`listener.onEvent(e)`)
  runs every lambda in the group (`Class::callees` through `Hierarchy::targets`), so their
  captures count in "reaches an object of its class another way". This is what keeps a
  callback that calls back into the object that invokes it (`Subscription.cancel` inside a
  listener) a Java re-entrant call and not a `RefCell` panic.
- **Clone out before calling**: a functional value read from a field and then called
  (`onClick.run()`) is cloned out of the borrow first: `let f = this.borrow().on_click.clone();
  f();`, since the body may reach the same object. The existing `bound_call` machinery (bind
  receiver and arguments first) does this when `borrows_here` counts the call as reaching the
  object; the closure is a cheap `Rc` clone.

### 1.4 Captures and threads

`analysis/threads.rs` (multi-threading, section 4.1) lists "a value captured by a Runnable,
Callable, Supplier ... passed to `new Thread`, an executor, ..." among its roots. With captures
as fields of a synthetic class, nothing special is needed: the lambda's class is a class whose
fields are its captures, the `Thread` constructor stores a `Runnable` in a thread-shared field,
so by the type-based closure rule the lambda class is thread-shared and so are the classes of
its fields. The generator then writes `Arc<dyn Fn() -> Result<(), Throwable> + Send + Sync>` for
the closure type and `Arc::clone` in the prelude; rustc checks `Send` (an `Rc` left behind is a
compile error, never a miscompilation). A closure that stays on one thread keeps `Rc`.

### 1.5 Captures in constructors and initializers

A struct-literal constructor builds the object as a plain value and `new` wraps it in `Rc`, so
there is no handle to capture while the constructor runs. A field initializer or constructor
statement that captures `this` (a lambda, `new Inner()`, an anonymous class) is rejected for now
("`this` captured in a constructor"). The JDK's collections create their inner iterators and
views lazily in methods (`HashMap.keySet()`, `ArrayList.iterator()`), so this blocks none of
them; the later fix is two-phase construction (build with the field `None`, assign after
`Rc::new`), not `Rc::new_cyclic`, so that the closure holds a strong handle like every other
capture.

## 2. Inner (non-static nested) classes

### 2.1 Representation

```java
class Names implements Iterable<String> {
    private final String[] items;
    private int size;
    Names(int capacity) { items = new String[capacity]; }
    void add(String s) { items[size++] = s; }
    public Iterator<String> iterator() { return new Cursor(); }

    class Cursor implements Iterator<String> {
        private int next;
        public boolean hasNext() { return next < size; }
        public String next() { return items[next++]; }
    }
}
```

```rust
struct Names {
    items: java::Array<Option<String>>,
    size: i32,
}

struct NamesCursor {
    outer: Rc<RefCell<Names>>,
    next: i32,
}

impl Names {
    fn new(capacity: i32) -> Names { Names { items: java::new_array(capacity), size: 0 } }
    fn add(&mut self, s: String) {
        let i = self.size;
        self.size = self.size.wrapping_add(1);
        java::put(&self.items, i, Some(s));
    }
    fn iterator(this: &Rc<RefCell<Names>>) -> Rc<RefCell<NamesCursor>> {
        Rc::new(RefCell::new(NamesCursor::new(this.clone())))
    }
}

impl NamesCursor {
    fn new(outer: Rc<RefCell<Names>>) -> NamesCursor { NamesCursor { outer, next: 0 } }
    fn has_next(&self) -> bool { self.next < self.outer.borrow().size }
    fn next(&mut self) -> String {
        let i = self.next;
        self.next = self.next.wrapping_add(1);
        java::non_null(java::at(&self.outer.borrow().items, i))
    }
}
```

- The hidden reference is the first constructor parameter, `outer`, named `outer` in Rust
  (javac's `this$0`); a user field named `outer` is renamed `outer_` as `base` is today.
  Every constructor of the inner class sets it; `this(..)` passes it on.
- `iterator()` returns the concrete `Rc<RefCell<NamesCursor>>` when `Cursor` is the only
  concrete class under `Iterator` in the program, and the `Iterator` enum otherwise (object
  model's `Rep`). `Names::iterator` takes the handle form because it captures `this` (1.3).
- The outer's private fields are read through `outer` with no access check: Rust modules make
  them visible (the inner class lives in the same module as the outer).
- An inner class may be shared, immutable or owned by the usual per-class decision; the
  `outer` field is a handle, so an inner class is never a plain value unless the outer is
  immutable and the inner is never shared (rare; `Rc<Outer>` then).

### 2.2 Construction

| Java | Rust |
| --- | --- |
| `new Cursor()` inside an instance method of `Names` | `NamesCursor::new(this.clone())`, where `this` is the handle (1.3) |
| `names.new Cursor()` | `NamesCursor::new(names.clone())` (a null `names` throws `NullPointerException` through `java::non_null`) |
| `new Cursor()` inside another inner class of `Names` | `NamesCursor::new(self.outer.clone())` |
| `new Cursor()` inside a lambda inside `Names` | the lambda captures `this` and passes it |
| `new Cursor()` in a static method | a javac error; cannot occur |

`ExprKind::New::outer` holds the enclosing-instance expression in each case; lowering fills the
implicit one from the scope (`This`, `OuterThis { class }`, or the capture).

### 2.3 Access to the outer

`Outer.this` is `ExprKind::OuterThis { class }`; an unqualified name that javac resolves to a
member of an enclosing class (`size`, `get(next++)`, `items`) is lowered as a field read or
method call on `OuterThis`. The generator writes the chain of `outer` fields from the current
class to `class` (`self.outer`, `self.outer.borrow().outer`), each step a handle read in the
outer's form (`borrow()` for `Rc<RefCell>`, nothing for `Rc<C>`, `lock()` for a thread-shared
outer). A call on the outer is a call on a handle, with the usual borrow discipline (`bound_call`
when the arguments could reach the outer). An inner class that shadows an outer field (`name` in
both `Ring` and `RingIterator`) is resolved by javac: `name` is `self.name`, `Ring.this.name` is
`self.outer.borrow().name`.

### 2.4 Inner classes in class trees

- **Inner extends inner of the same outer** (`LinkedKeyIterator extends LinkedHashIterator`,
  `ListItr extends Itr`): `outer` lives in the fields struct of the root inner class; the
  subclass embeds it as `base`, the generator writes `self.base.outer`, and the subclass's
  constructor passes the outer through `super(..)` as its first argument (javac does the same).
- **Inner of a different outer extends an inner class** (rare): each inner level with a distinct
  outer class has its own `outer` field; `OuterThis { class }` picks the right one by class.
- **Inner classes implementing interfaces or extending abstract classes** (iterators, the views
  `KeySet extends AbstractSet`): variants of the interface enum as any class; the inherited
  bodies live where the object model puts them.
- `is_inner` in `analysis/hierarchy.rs` today means "a class with subclasses"; it is renamed
  `has_fields_struct` to free the word.

### 2.5 Naming and modules

A nested class is already named by concatenation (`Bank.InsufficientFunds` is
`BankInsufficientFunds`); inner classes follow (`NamesCursor`, `HashMapKeyIterator`), in the
module of their package next to the outer. Local classes are `<Outer><Name>` with the method's
name appended only on a clash of two local classes of one name (`MainLabeler`). Anonymous
classes are `<Supertype>In<Method>` (`ComparatorInMain`, `IteratorInIterator`), numbered on a
clash (`IteratorInIterator2`). Lambda classes that stay structs are `<Interface>LambdaIn<Method>`
with the same numbering; closure-form lambdas have no name.

### 2.6 Generic outers

Under erased generics (what exists) nothing is special: `Ring<T>.RingIterator` erases like
`Ring`. Under real generics (object model milestone 6) an inner class of a real generic class is
excluded in the first version (the outer stays erased, "inner classes of generic classes" in the
disqualifying conditions); the later form is `struct RingRingIterator<T> { outer:
Rc<RefCell<Ring<T>>>, .. }` with the outer's parameters repeated.

## 3. Local and anonymous classes

### 3.1 Local classes

```java
String prefix = "item-";
class Labeler { String label(int n) { return prefix + n * n; } }
Labeler labeler = new Labeler();
```

```rust
struct MainLabeler { prefix: String }
impl MainLabeler {
    fn new(prefix: String) -> MainLabeler { MainLabeler { prefix } }
    fn label(&self, n: i32) -> String { format!("{}{}", self.prefix, n.wrapping_mul(n)) }
}
let labeler = MainLabeler::new(prefix.clone());
```

The captures (1.1) are fields and trailing constructor parameters of every constructor (javac's
`val$prefix`); `new Labeler()` passes them from the scope. A local class in an instance method
that uses the enclosing instance has `outer` too, first. Local classes may extend classes,
implement interfaces and be extended by other local classes of the same block; they are concrete
classes of the hierarchy like any other and get enum variants where they flow. Local records,
enums and interfaces are implicitly static: no captures, no `outer`.

### 3.2 Anonymous classes

```java
Comparator<Person> byFirstLength = new Comparator<Person>() {
    public int compare(Person a, Person b) {
        int diff = a.getFirst().length() - b.getFirst().length();
        return diff != 0 ? diff : a.getFirst().compareTo(b.getFirst());
    }
};
```

When `Comparator` is an enum in the program (a named class implements it somewhere, as the
JDK's `Comparators.NullComparator` does once `nullsFirst` is reached):

```rust
struct ComparatorInMain;
impl ComparatorInMain {
    fn compare(&self, a: &Rc<Person>, b: &Rc<Person>) -> i32 {
        let diff = java::length(&a.get_first()).wrapping_sub(java::length(&b.get_first()));
        if diff != 0 { diff } else { java::compare_to(&a.get_first(), &b.get_first()) }
    }
}
enum Comparator { InMain(Rc<ComparatorInMain>), NullComparator(Rc<ComparatorsNullComparator>), /* .. */ }
let by_first_length = Comparator::InMain(Rc::new(ComparatorInMain));
```

When `Comparator` is a closure type (only lambda-like values flow into it, section 4.2), the
anonymous class is created directly in a conversion to its interface, so it *is* lambda-like and
is written as a closure (4.4 shows it). The general rules:

- An anonymous class is a synthetic concrete class whose supertype is the named type: it
  implements the interface or extends the class. Its one constructor takes `outer` (in an
  instance context), the captures, and the arguments of the `new`, which it passes to
  `super(..)`: `new Thread("worker") { public void run() { .. } }` is `struct ThreadInMain {
  base: ThreadFields, .. }` built from `Thread`'s fields constructor; `new LinkedHashMap<>(16,
  0.75f, true) { protected boolean removeEldestEntry(..) { .. } }` is a subclass with a
  `super(16, 0.75, true)` constructor and an override the object model dispatches to.
- Fields with initializers and instance initializer blocks of the anonymous class run in its
  constructor as for any class (`private int current = from;` where `from` is a captured outer
  field read through `outer`).
- `this` inside the anonymous class is the anonymous object; the enclosing instance is
  `OuterThis`; a method of the enclosing class called unqualified goes through `outer`.
- Joining the closed world: an anonymous class is a concrete class in `Hierarchy::concretes`, so
  it gets a variant of every type it flows into, its methods join the override groups, and
  `instanceof`/casts see it. It is never named by the program, so no `Cast` to it exists and it
  is only ever reached through its supertype.
- An anonymous class that extends a class and overrides a method called from that class's
  constructor gets the object model's second constructor form (`docs/object-model.md` 1.4): the
  superclass's constructor body runs on the whole object, so it calls the anonymous class's
  override.

### 3.3 Anonymous classes with state as closures

An anonymous `Iterator` with a field (`private int i;`) is not a functional interface
implementation (two abstract methods) and stays a struct. An anonymous class of a *functional*
interface with its own fields stays a struct too in the first version: the closure form would
need a `Cell`/`RefCell` inside the closure, which is correct but not plainer. Only anonymous
classes without own fields take the closure form.

## 4. Lambdas, method references and functional interfaces

### 4.1 Lowering: a lambda is a class

javac gives a `LAMBDA_EXPRESSION` node its target type in `javaType`
(`java.util.function.Function<java.lang.Integer,java.lang.Integer>`) but not the method it
implements. `crates/translator/java/JavaTree.java` gets one addition: for `LAMBDA_EXPRESSION`
and `MEMBER_REFERENCE` nodes a field `descriptor` with the functional interface's single
abstract method (its owner, name, erased type, and the type after substitution through
`Types.asMemberOf(targetType, method)`), found among `Elements.getAllMembers` of the target
interface as the one abstract method not overriding a method of `Object`. For a
`MEMBER_REFERENCE` the node already has `mode` (`INVOKE`/`NEW`), `qualifierExpression` and the
`symbol` of the referenced method; the kind is derived: `NEW` on an array type is an array
constructor, `NEW` otherwise a constructor, a static symbol is static, a qualifier whose symbol is
a type (`String::length`) is unbound, anything else (`this::m`, `super::m`, `System.out::println`,
`list::add`) is bound.

`lower/lambdas.rs` creates for each node a synthetic `TypeDecl` (`Origin::Lambda`) implementing
the interface, with one method whose parameters are the lambda's (typed by the *substituted*
descriptor types, with `Cast` nodes to and from the erased declared types exactly as the
object model's bridges do for erased generics) and whose body is the expression (`return e;`)
or block; captures per section 1. A method reference's body is the one call:

| Kind | Body of the synthetic method |
| --- | --- |
| static `Util::twice` | `return Util.twice(a, b);` |
| bound `prefix::concat`, `System.out::println`, `this::helper` | the receiver is evaluated once at creation into a capture `receiver` (a null receiver throws `NullPointerException` there, as Java does); body `return receiver.concat(a);` |
| unbound `String::length`, `Person::getAge` | the first parameter is the receiver: `return a.length();` |
| constructor `StringBuilder::new`, `ArrayList::new` | `return new ArrayList<>(a);` with the overload javac resolved |
| array constructor `int[]::new` | `return new int[n];` |

Varargs, boxing and widening in the descriptor-to-method adaptation are javac's resolved
choices; lowering reads the `symbol` and inserts the conversions `convert_args` already inserts.

`ExprKind::Lambda { class }` is the creating expression. Its `ty` is the interface type.
Everything downstream treats the synthetic class as a class until the representation facts say
otherwise.

### 4.2 Which functional interfaces are closure types

`analysis/closures.rs` computes `Facts::closure_types: HashSet<usize>` (interface indexes) and
`Facts::closure_params: HashSet<(usize, usize)>`, `Facts::closure_locals: HashSet<(usize,
String)>`. An interface `I` is a closure type when all hold:

1. `I` is functional: exactly one abstract method after excluding `Object`'s, over `I` and the
   interfaces it extends (`UnaryOperator<T> extends Function<T, T>` adds none). Default and static
   methods are allowed (4.6).
2. Every value of `I` (in the `flows` sense of the object model) comes from a lambda or method
   reference, or from a `new` of an anonymous class without fields of its own whose `new` is
   immediately converted to `I` or a sub-interface of it. A named class implementing `I`
   (`Counter implements Listener<Event>`, `Comparators.NullComparator`), or an anonymous class
   with fields, makes `I` an enum.
3. No call of `equals`, `hashCode`, `getClass` or `toString` on a value of `I` (identity through
   `Object` is `Rc::ptr_eq` on the variant and would be fine; the condition is kept until a
   corpus program needs it dropped).
4. Every sub-interface of `I` with values is a closure type too (the same closure signature after
   substitution); otherwise both are enums.

Everything else is the object model's enum form with one variant per lambda class. The decision
is per interface and program-wide, like `Rep`; both forms coexist in one program (`Runnable` a
closure type, `Comparator` an enum, when the program reaches `nullsFirst`).

### 4.3 Forms of a closure-typed value

| Where the value lives | Rust | Lambda written as |
| --- | --- | --- |
| a parameter the method only calls, or passes to such a parameter, and that no lambda the method creates captures (`closure_params`) | `impl FnMut(A) -> R` (generic parameter) | inline: `map_all(&words, \|s\| java::length(s))` |
| the same, but the method passes a new lambda wrapping it to itself (recursion through wrappers) | `&mut dyn FnMut(A) -> R` | `&mut \|x\| ..` |
| a local that is only called and never stored, returned, captured or passed to a stored place (`closure_locals`) | the closure's own type: `let f = \|x\| ..;` | inline |
| a field, a static, a return value, an element, a stored parameter, a captured value | `Rc<dyn Fn(A) -> R>`, `Arc<dyn Fn(A) -> R + Send + Sync>` when the lambda class is thread-shared | `Rc::new({ prelude; move \|x\| .. })` |
| an `Object`-typed place | the variant `Object::<Interface>(Rc<dyn Fn ..>)` of the `Object` enum, with identity `equals`, identity `hash_code` and `Main$$Lambda@hash` for `toString` | wrapped |

- `FnMut` for parameters, `Fn` for stored values. A Java lambda cannot assign a captured local,
  so its body mutates only through handles (`RefCell`, `Locked`) and is `Fn`; `FnMut` on
  parameters allows the borrowed `this` form: a lambda capturing `this` passed to an `impl
  FnMut` parameter borrows `self` (`|x| self.total += x` captures the field `self.total` alone
  in Rust 2021, so the receiver `self.list.borrow()` of the same statement does not conflict) and
  the method keeps `&mut self`. A stored lambda captures `this` as a handle (1.3).
- A `Vec` array captured by a parameter-form lambda is borrowed (`&a`, `&mut a`), the arrays
  analysis treating the closure body as part of the statement; a stored lambda shares it (1.2).
- A stored `Rc<dyn Fn>` passed to an `impl FnMut` parameter is `&*f`.
- The argument and return types inside `dyn Fn(..)` follow the override group's signature
  (`borrowed_objects`, `borrowed_params`, nullness), so `Comparator<Person>` is `Fn(&Rc<Person>,
  &Rc<Person>) -> i32` when both lambdas only use their parameters through, and `Fn(Rc<Person>,
  ..)` otherwise; `Function<String, Integer>` is `Fn(String) -> i32` under the boxing analysis's
  `i32` for an `Integer` whose identity is never observed, `Fn(String) -> Boxed<i32>` otherwise,
  `Fn(String) -> Option<i32>` when some lambda may return null.
- **Type aliases**: a closure type that appears in more than one signature gets
  `type <Interface>[Of<Args>] = Rc<dyn Fn(..)>;` next to the interface's module (`type Runnable =
  Rc<dyn Fn() -> Result<(), Throwable>>;`, `type ListenerOfEvent = Rc<dyn Fn(&Event)>;`), named
  as the object model names instantiation variants.
- `Box<dyn Fn>` for a field that is the value's only holder is a refinement through
  `Facts::moves` (the lambda is moved into the field and never cloned); not in the first version.

### 4.4 Generics and bridges

A closure type is computed from the *substituted* javac type at each declaration site, not per
interface. `Function<Integer, Integer>` in `main` is `Rc<dyn Fn(i32) -> i32>`; `Function<? super
K, ? extends V>` inside the erased `Map.computeIfAbsent` is `Rc<dyn Fn(Object) -> Object>`;
`Function<T, R>` inside the real generic `map_all<T, R>` is `impl FnMut(&T) -> R`. Where a value
of one closure type flows into a place of another, the generator writes the bridge javac
would: `Rc::new(move |o: Object| Object::ArrayList(f(o.to_java_string())))`. This keeps the
program's own lambdas typed while the JDK's wildcard-heavy code stays erased, and disappears
site by site as generic classes become real. A lambda whose body javac typed with the
substituted types gets `Cast` nodes at the parameters and the result when it converts to an
erased closure type (`move |x: Object| { let x = x.to_integer(); Object::Integer(x.wrapping_mul(2)) }`).

### 4.5 Calls

A call of the abstract method on a closure-typed value is a call of the closure: `f.apply(x)` is
`f(x)`, `cmp.compare(a, b)` is `cmp(&a, &b)`, `r.run()` is `r()` or `r()?`. On an enum-typed
value it is the dispatcher as for any interface (`listener.on_event(&e)` matching the variants,
a lambda variant's arm calling the struct's method). A functional value read from a field is
cloned out before the call (1.3).

### 4.6 Default and static methods of closure-typed interfaces

They are translated from the JDK's source (or the program's) like any method, but their home is
the closure type: default methods become methods of a generated extension trait implemented for
the `Rc<dyn Fn ..>` type, static methods free functions of the interface's module. The
`this` of a default method is the closure itself, cloned into lambdas it creates.

```java
// java/util/Comparator.java
default Comparator<T> thenComparing(Comparator<? super T> other) {
    Objects.requireNonNull(other);
    return (Comparator<T> & Serializable) (c1, c2) -> {
        int res = compare(c1, c2);
        return (res != 0) ? res : other.compare(c1, c2);
    };
}
public static <T, U extends Comparable<? super U>> Comparator<T> comparing(Function<? super T, ? extends U> keyExtractor) {
    Objects.requireNonNull(keyExtractor);
    return (Comparator<T> & Serializable) (c1, c2) -> keyExtractor.apply(c1).compareTo(keyExtractor.apply(c2));
}
```

```rust
// erased form (what lands first): T is Object, U is Comparable
pub(crate) type Comparator = Rc<dyn Fn(&Object, &Object) -> i32>;

pub(crate) trait ComparatorMethods {
    fn then_comparing(&self, other: Comparator) -> Comparator;
    fn reversed(&self) -> Comparator;
}

impl ComparatorMethods for Comparator {
    fn then_comparing(&self, other: Comparator) -> Comparator {
        Objects::require_non_null(Object::Comparator(other.clone()));
        let this = self.clone();
        Rc::new(move |c1, c2| {
            let res = this(c1, c2);
            if res != 0 { res } else { other(c1, c2) }
        })
    }

    fn reversed(&self) -> Comparator {
        Collections::reverse_order_with(self.clone())
    }
}

pub(crate) fn comparing(key_extractor: Rc<dyn Fn(Object) -> Comparable>) -> Comparator {
    Objects::require_non_null(Object::Function(key_extractor.clone()));
    Rc::new(move |c1, c2| key_extractor(c1.clone()).compare_to(&key_extractor(c2.clone()).to_object()))
}
```

```java
people.sort(Comparator.comparing(Person::getLast).thenComparing(Person::getAge));
```

```rust
people.borrow_mut().sort(
    comparator::comparing(Rc::new(|p: Object| Comparable::String(p.to_person().get_last())))
        .then_comparing(comparator::comparing(Rc::new(|p: Object| Comparable::Integer(p.to_person().get_age())))),
);
```

`reversed()` calls `Collections.reverseOrder(this)`, which returns `new ReverseComparator2<>(cmp)`:
a named class implementing `Comparator`, so in a program that reaches it `Comparator` is an enum
and the lambdas above are variants (`Comparator::LambdaInComparing(Rc<ComparatorLambdaInComparing>)`
holding `key_extractor` as a field). The closure form above is what a program that uses only
`comparing`/`thenComparing`/`comparingInt` gets. Under real generics the same code is
`Rc<dyn Fn(&Rc<Person>, &Rc<Person>) -> i32>` with `|p: &Rc<Person>| p.get_last()` and no `Object`.

The `(Comparator<T> & Serializable)` cast: lowering drops marker interfaces (`java.io.Serializable`,
`java.lang.Cloneable`, `java.util.RandomAccess`) from intersection casts and `implements`
clauses, since they declare nothing; a lambda with two non-marker target types stays rejected.

### 4.7 Exceptions in lambdas

The lambda's synthetic method is in the override group of the descriptor, so
`analysis/exceptions.rs` applies as written: what escapes a lambda body is what escapes the
group, `observed` is what the callers of `f.apply`, `r.run`, `c.call` observe, and the group
returns `Result<R, Throwable>` exactly when some caller observes. Then the closure type is
`Rc<dyn Fn(A) -> Result<R, Throwable>>`, the lambda body is written as a `Result` method (`Ok(..)`
tail, `?` inside), and every call is `f(a)?` (or `.unwrap_or_else(java::uncaught)` in a method
that does not return `Result`). A thread's entry observes everything (multi-threading, section 6),
so a `Runnable` given to a `Thread` returns `Result` whenever its body can throw; a user interface
`interface Parser { Node parse(String s) throws SyntaxError; }` is the same case with a checked
exception. `Preconditions.outOfBoundsCheckIndex(BiFunction<String, List<Number>, X> oobef, ..)`
calls the function to *build* the exception and then throws it: the lambda returns
`Throwable` (an `X extends RuntimeException` erases to the `Throwable` enum of the exceptions
design) and throws nothing itself.

### 4.8 Recursion and self-capture

A lambda cannot capture itself through a local (not effectively final). Through a field it reads
the field at call time: `static Function<Integer, Integer> FIB = n -> n < 2 ? n : FIB.apply(n - 1) +
FIB.apply(n - 2);` is a static cell holding the `Rc<dyn Fn>`, read on each call
(`FIB.with(|f| f.borrow().clone())(n - 1)`); an instance field `this.next = x -> this.next.apply(x)`
captures `this` and makes an `Rc` cycle, which the `Rc` design already accepts. A lambda-typed
field set after construction is `Option<Rc<dyn Fn ..>>` by nullness as any object field.

### 4.9 The main cases

**A lambda capturing a local and `this`, passed to a borrowed parameter:**

```java
class Tally {
    int total;
    final List<String> seen = new ArrayList<>();
    void addAll(List<Integer> xs, int weight) {
        xs.forEach(x -> { total += x * weight; seen.add("x" + x); });
    }
}
```

```rust
impl Tally {
    fn add_all(&mut self, xs: &Rc<RefCell<ArrayList>>, weight: i32) {
        xs.borrow().for_each(|x| {
            let x = x.to_integer();
            self.total = self.total.wrapping_add(x.wrapping_mul(weight));
            self.seen.borrow_mut().add(Object::String(format!("x{x}")));
        });
    }
}
```

`Iterable.forEach(Consumer<? super T> action)` is a JDK default method whose parameter is only
called, so `for_each` takes `impl FnMut(Object)` on the `Iterable` enum (or on `ArrayList` when
it is the single implementor) and the lambda is inline, borrowing `self.total` and `self.seen`
disjointly; `weight` is `Copy`. Had `addAll` stored the lambda, it would be `Rc::new({ let this
= this.clone(); move |x| { this.borrow_mut().total = ..; } })` and `add_all(this: &Rc<RefCell<Tally>>, ..)`.

**`list.forEach(System.out::println)`:** `list.borrow().for_each(|x| java::println!("{}", x))`;
`x` is `Object` under erased generics and prints through `Object`'s `Display`; under real
generics `|x: &String|`. `System.out` is a constant receiver, so the bound reference captures
nothing.

**A lambda stored in a field and called later:**

```java
class Button {
    private Runnable onClick;
    void setOnClick(Runnable r) { onClick = r; }
    void click() { if (onClick != null) onClick.run(); }
}
Button b = new Button();
int[] clicks = {0};
b.setOnClick(() -> { clicks[0]++; System.out.println("clicked " + clicks[0]); });
b.click(); b.click();
```

```rust
pub(crate) type Runnable = Rc<dyn Fn()>;
struct Button { on_click: Option<Runnable> }
impl Button {
    fn set_on_click(&mut self, r: Runnable) { self.on_click = Some(r); }
    fn click(this: &Rc<RefCell<Button>>) {
        let on_click = this.borrow().on_click.clone();
        if let Some(on_click) = on_click { on_click(); }
    }
}

let b = Rc::new(RefCell::new(Button::new()));
let clicks = java::share(vec![0]);
b.borrow_mut().set_on_click(Rc::new({
    let clicks = clicks.clone();
    move || {
        java::modify(&clicks, 0, |v| v.wrapping_add(1));
        java::println!("clicked {}", java::at(&clicks, 0));
    }
}));
Button::click(&b);
Button::click(&b);
```

`click` takes the handle form because the `Runnable` group's lambdas may reach a `Button` (this
one does not, but a lambda given to `setOnClick` elsewhere might: the analysis is per group), and
clones the closure out before calling it. `clicks` is shared because the capture is a second
name for it. `Runnable.run` returns no `Result` here since nothing observes an exception.

**A `Runnable` given to a `Thread`** (multi-threaded program):

```java
Counter c = new Counter();
Thread t = new Thread(() -> { for (int i = 0; i < 1000; i++) c.increment(); });
t.start();
t.join();
System.out.println(c.get());
```

```rust
pub(crate) type Runnable = Arc<dyn Fn() -> Result<(), Throwable> + Send + Sync>;

let c = Arc::new(java::Locked::new(Counter::new()));
let t = Thread::new_runnable(Arc::new({
    let c = c.clone();
    move || {
        for _i in 0..1000 {
            c.lock().increment();
        }
        Ok(())
    }
}));
t.start()?;
t.join()?;
java::println!("{}", c.lock().get());
```

`Thread` is the synthetic class of the multi-threading design; its `task` field holds a
`Runnable`, the field is thread-shared, so the lambda class and `Counter` are thread-shared by the
type-based rule and the closure is `Send + Sync`, which rustc verifies. `run()` returns `Result`
because the thread entry observes it. `java::runnable(..)` of the multi-threading examples is
this `Arc::new` of the prelude block; no runtime helper is needed.

**An anonymous `Comparator` where `Comparator` is a closure type:**

```rust
let by_first_length: Comparator = Rc::new(|a: &Rc<Person>, b: &Rc<Person>| {
    let diff = java::length(&a.get_first()).wrapping_sub(java::length(&b.get_first()));
    if diff != 0 { diff } else { java::compare_to(&a.get_first(), &b.get_first()) }
});
by_first.borrow_mut().sort(by_first_length);
```

## 5. for-each over `Iterable`

Java's enhanced `for` over a non-array is `for (Iterator<T> it = c.iterator(); it.hasNext(); ) {
T x = (T) it.next(); body }`. The translation keeps the protocol, because `hasNext`/`next` are
translated code with Java's behaviour (`ConcurrentModificationException`, the quirk that
removing the second-to-last element ends an `ArrayList` loop, `NoSuchElementException`).

**General form**, when `has_next` or `next` returns `Result`:

```rust
let it = Names::iterator(&names);          // or `names.iterator()` on an enum / `&self` form
while it.borrow().has_next()? {
    let s = it.borrow_mut().next()?;
    ..
}
```

**Idiomatic form**, when neither returns `Result` (the common case):

```rust
for s in java::iterate(Names::iterator(&names)) {
    ..
}
```

```rust
// crates/runtime/src/iterate.rs
pub trait Iterating { type Item; fn has_next(&self) -> bool; fn next(&mut self) -> Self::Item; }
impl<C: Iterating> Iterating for Rc<RefCell<C>> { .. }   // and for Arc<Locked<C>>
pub fn iterate<I: Iterating>(mut it: I) -> impl Iterator<Item = I::Item> {
    std::iter::from_fn(move || it.has_next().then(|| it.next()))
}
```

The generator writes `impl java::Iterating for NamesCursor` (`&self`/`&mut self` forwarding to
the translated methods) for every iterator class that a for-each reaches, and `impl
java::Iterating for Iterator` for the enum (arms borrow per variant). Rust's `for` calls the
adapter once per step, which calls `has_next` then `next`: the same call sequence as Java's loop,
including after `break`, so the body may change the collection and the translated iterator
throws what Java throws. The loop variable's `Cast` from the erased element type is the body's
first line (`let s = s.to_java_string();`), omitted under real generics. The `Iterator` enum has
variants for every iterator class that flows (the JDK's `ArrayList.Itr`, `HashMap.KeyIterator`,
the program's inner classes, anonymous iterators); when a program has one, `it` is that class's
object and no enum exists.

`impl Iterator for NamesCursor` and `for s in &names`, for an iterator class whose
`hasNext`/`next` only read its fields and the outer, is the thorough-mode row "a class
implementing Iterator → `impl Iterator`" and the `ArrayList`-as-`Vec` loop of
`docs/lists-and-streams.md`, not this plan. `Iterable.forEach`, `Iterator.forEachRemaining` and
`Map.forEach` are default methods translated from the JDK over the same protocol. In single-file
mode `java.lang.Iterable` and `java.util.Iterator` are runtime interfaces with their abstract
methods, as `Comparable` is today; their default methods need the JDK sources.

## 6. Interaction with the planned work

- **Lists and streams** (`docs/lists-and-streams.md`): a stream pipeline becomes an iterator
  chain with inline closures: `filter(p)` is `.filter(|x| p(x))` for a stored `p` and
  `.filter(|x| ..)` for a lambda written at the call, the parameter form of 4.3 applied to the
  stream operations. `Collectors` stay translated: `CollectorImpl` holds `Supplier`,
  `BiConsumer`, `BinaryOperator` and `Function` *fields*, so those are stored closures and
  `Collectors.toList()` is `CollectorImpl::new(Rc::new(|| ..), Rc::new(|l, x| ..), ..)`.
- **Thread escape** (`docs/multi-threading.md`): its open question on the lambdas'
  representation is answered by 1.4: a lambda is a class whose fields are its captures, so the
  roots of its section 4.1 read `TypeDecl::captures` and `outer`; a thread-shared functional
  value is `Arc<dyn Fn .. + Send + Sync>`; `java::runnable` is not needed. Its executors
  milestone (`Callable`, `Future`, `CompletableFuture.thenApply`, `UncaughtExceptionHandler`) is
  closure types plus the stored form.
- **Exceptions**: nothing new (4.7). The `Throwable` enum gains variants for exception classes
  that lambdas create; `catch` inside a lambda body is the labeled block as anywhere.
- **Erased and real generics**: 4.4; a functional interface never needs to be "real" as a
  class does, since the closure type is per value; the generic *methods* taking them
  (`map_all<T, R>`) follow the object model's milestone 6 conditions, with `impl FnMut(&T) -> R`
  as the parameter.
- **Thorough mode** makes more lambdas plain closures: the points-to analysis replaces the
  type-based "may reach an object of its class" by per-site reachability, so more `this`
  captures borrow and more methods keep `&self`; a named class implementing a functional
  interface whose objects are only ever used through it (`ReverseComparator2`,
  `NullComparator`) is inlined into a closure, which makes `Comparator` a closure type even when
  `reversed()` is reached; `Box<dyn Fn>` from the moves analysis; `impl Iterator` for pure
  iterator classes; `Comparator` values that are `comparing` chains over keys become
  `sort_by_key`/`Ord` (its "`Comparable`, `Comparator` → `impl Ord`" row); listener interfaces
  with one implementation become direct calls.

## 7. Work split for two implementers

**Nested-classes implementer**: captures (the shared code), inner, local and anonymous
classes, for-each over `Iterable`.
**Lambdas implementer**: lambdas, method references, functional interfaces, closure types,
their default and static methods, exceptions in lambdas.

### 7.1 Ownership

| Module | Owner | Content |
| --- | --- | --- |
| `ast.rs` additions of 1.1 (`outer`, `captures`, `Origin`, `Capture`, `New::outer`, `OuterThis`) | nested-classes | landed in the first commit of the first milestone, before anything else, so both build on it |
| `ast.rs` `ExprKind::Lambda`, `Origin::Lambda` | lambdas | added at the end of the enum in one small commit right after the above merges |
| `lower/captures.rs` | nested-classes | scope stack, boundaries, capture lists, `OuterThis` lowering, implicit enclosing instances |
| `lower/inner.rs` | nested-classes | inner, local and anonymous class declarations and constructors, naming (2.5) |
| `lower/objects.rs`, `lower/hierarchy.rs` | nested-classes | the `"CLASS" => unsupported(.., "inner classes")` and `"local classes"` arms replaced; synthetic classes registered |
| `lower/lambdas.rs` | lambdas | lambda and member-reference nodes into synthetic classes, the five reference kinds, descriptor types and `Cast`s |
| `lower/functional.rs` | lambdas | functional-interface detection, runtime functional interfaces for single-file mode (`Runnable`, `java.util.function.*`, `Comparator`, `Iterable`/`Iterator` abstract methods), marker-interface dropping |
| `crates/translator/java/JavaTree.java` | lambdas | the `descriptor` field (4.1) |
| `analysis/captures.rs` | nested-classes | `this`-as-value facts (1.3): which methods need the handle form because of captures; owned-class exclusions |
| `analysis/borrowing.rs`, `analysis/owned.rs` | nested-classes | consult the above; `callees` through functional groups is the object model's `Class::callees` and needs no change |
| `analysis/closures.rs` | lambdas | `closure_types`, `closure_params`, `closure_locals`, closure signatures per group, bridges needed, `Object` variants for closures |
| `analysis/hierarchy.rs` | nested-classes | synthetic classes as concretes, `is_inner` renamed `has_fields_struct`; lambda classes need no change |
| `analysis/threads.rs` (when it exists) | neither | reads `captures`/`outer`; the multi-threading implementer |
| `codegen/captures.rs` | nested-classes | the prelude block, struct fields and constructor parameters for captures, `outer` chains |
| `codegen/inner.rs` | nested-classes | inner, local and anonymous class items, `New` with `outer`, `OuterThis` reads |
| `codegen/foreach.rs`, `crates/runtime/src/iterate.rs` | nested-classes | the two loop forms, `impl java::Iterating` items |
| `codegen/closures.rs` | lambdas | closure types and aliases, inline and `Rc::new` forms, calls, bridges, extension traits and free functions for default and static methods, `Object` arms |
| `codegen/dispatch.rs`, `codegen/objects.rs` | shared, small edits | lambda-variant arms (lambdas), `outer` handle reads (nested-classes); coordinate by touching different functions |
| `program/reach.rs`, `program/survey.rs` | both | anonymous and local classes reached through their `new` (nested-classes); lambda bodies reached with their enclosing method and descriptor calls reaching lambda bodies (lambdas); the survey rows removed as each lands |
| `README.md` "Current state" | both | each milestone appends its paragraph |

### 7.2 Order and stubs

1. The nested-classes implementer lands the AST additions and `lower/captures.rs` first
   (milestone "Captures and local classes", below), within the first days. Until it merges, the
   lambdas implementer works on non-capturing lambdas and method references (static references,
   lambdas over their parameters only, unbound references), with `captures` empty and `outer`
   `None` as a stub, and rejects a capturing lambda with "lambda captures" so the corpus stays
   honest.
2. The lambdas implementer's `ExprKind::Lambda` commit merges right after; from then on both
   branches touch disjoint files except the shared small edits of 7.1.
3. Merge order into `main`: captures and local classes → non-capturing lambdas (object form) →
   inner classes → closure forms → anonymous classes → lambda captures → for-each over
   `Iterable` → JDK functional interfaces end to end. Each merge has the corpus green on both
   modes; the target programs are re-run (`targets/run.sh`) and the README table updated at the
   end of each.

### 7.3 Milestones: nested-classes implementer

Each milestone is mergeable alone: `cargo fmt`, `cargo clippy --all-targets -q -- -D warnings`,
`cargo test -q` green, `scripts/check.sh` passing, the README's "Current state" extended, the
survey's feature rows adjusted.

**Captures and local classes.** AST of 1.1; `lower/captures.rs`; local classes with captures
as fields and trailing constructor parameters; `new Labeler()` passing the scope's values;
captures of captures through nested local classes; `this` capture rejected in constructors
(1.5); `analysis/captures.rs` with the this-as-value facts and their use in `borrowing.rs` and
`owned.rs`; `codegen/captures.rs`. Shape tests: `a_local_class_holds_its_captures_as_fields`,
`a_captured_array_is_shared`, `a_method_whose_this_is_captured_keeps_the_handle`. Corpus:
`corpus/features/LocalClasses.java` (a local class with captured locals of every kind: int,
String, an object, an array that it mutates, a captured parameter; two local classes of the same
name in two methods; a local class extending a program class and one implementing a program
interface; a local record), `corpus/features/CapturedLocals.java` (captures of captures, a local
class whose method creates another local class, a capture used after the class is created, a
captured local that is dead after the capture). Unblocks: the `Labeler` part of
`InnerClassIterator`.

**Inner classes.** `outer`, implicit and explicit enclosing instances, `Outer.this`, unqualified
outer members, shadowing, inner classes in class trees (2.4), implementing interfaces, naming,
owned/immutable/shared decisions, the `is_inner` rename. Shape tests:
`an_inner_class_holds_the_outer_handle`, `outer_this_is_the_outer_field_chain`,
`inner_subclasses_share_one_outer_field`. Corpus: `corpus/features/InnerClasses.java` (an inner
class reading and writing outer fields, `outer.new Inner()`, two inner classes of one outer
calling each other through the outer, an inner class with its own constructor and `this(..)`,
shadowing), `corpus/features/InnerClassTrees.java` (an abstract inner class with two inner
subclasses, dispatched through an interface; an inner class extending a static nested class),
`corpus/features/InnerIterators.java` (a user list with an inner iterator class used by hand
with `hasNext`/`next`, two cursors over one list at once, `remove`),
`corpus/programs/TextIndex.java` (a word index whose inner `Entry` and `Cursor` classes read the
index's tables). Unblocks: `GenericContainers` (the inner `Node`), the inner counter of
`LruCacheLinkedHashMap`; in whole-program mode, `HashMap`'s and `ArrayList`'s iterators and views
become translatable (the collections targets then depend on for-each and lambdas).

**Anonymous classes.** Implementing interfaces and extending classes with constructor
arguments, captures and `outer` through the shared code, field initializers and initializer
blocks, variants and override groups, naming (2.5), `getClass().getSimpleName()` of an anonymous
class is `""` (as Java). Shape tests: `an_anonymous_class_is_a_variant_of_its_interface`,
`an_anonymous_subclass_passes_constructor_arguments_to_super`. Corpus:
`corpus/features/AnonymousClasses.java` (an anonymous comparator-like program interface, an
anonymous iterator with a field, an anonymous class capturing locals and the enclosing instance,
two anonymous classes of one interface in one method, an anonymous class inside an inner class
reaching `Outer.this`), `corpus/features/AnonymousSubclasses.java` (an anonymous subclass of an
abstract program class with constructor arguments and an instance initializer, overriding a
method the base calls from a non-constructor method), `corpus/programs/EventQueue.java` (handlers
as anonymous classes of a program interface, registered in a list and run in order, one
unregistering itself). Unblocks: `DocumentVisitor` (anonymous `Visitor`), the anonymous listener
of `EventListeners`, the anonymous `Comparator` of `ComparatorSorting` (with the lambdas work),
the anonymous subclass of `LruCacheLinkedHashMap`.

**For-each over `Iterable`.** The two loop forms, `impl java::Iterating` items, the runtime's
`iterate.rs`, `Iterable`/`Iterator` as runtime interfaces in single-file mode, `Iterator.remove`
through the protocol, the loop variable's cast. Shape tests:
`for_each_over_an_iterable_is_java_iterate`, `a_throwing_iterator_keeps_the_while_loop`.
Corpus: `corpus/features/ForEachIterable.java` (for-each over a user `Iterable` with an inner
iterator, over an anonymous iterator, nested for-each over two iterables, `break` and `continue`
inside, a body that calls `size()` on the collection), `corpus/features/IteratorRemove.java` (a
user list whose iterator supports `remove`, used from a while loop and after a for-each that
throws `ConcurrentModificationException` caught by the program), and in whole-program mode
`corpus/projects/collections-foreach` (for-each over `ArrayList`, `HashMap.keySet()`,
`entrySet()`, `LinkedHashMap`, with the modification quirks). Unblocks: `InnerClassIterator` (with
`forEach` from the lambdas work), `ListOperations`, `WordFrequency`, `IntegerKeyedGroups`,
`DequeBfs` (Deque iteration), `TextAdventure`, `InventoryManagement`, `ScannerGrades` (their
for-each loops over collections), all in whole-program mode.

### 7.4 Milestones: lambdas implementer

**Non-capturing lambdas and method references as objects.** `JavaTree.java` `descriptor`;
`lower/functional.rs` and `lower/lambdas.rs`; synthetic lambda classes in the enum form of the
object model (a variant per lambda, struct without fields, the method with `Cast`s to the
erased descriptor); all five method reference kinds (bound ones with a receiver that is a
static or a parameter, since captures are stubbed); user functional interfaces; the JDK's
functional interfaces as runtime interfaces in single-file mode (abstract method only); marker
interfaces dropped from intersection casts; `program/reach.rs` reaching lambda bodies. Shape
tests: `a_lambda_is_a_variant_of_its_interface`, `a_method_reference_is_a_lambda_class`,
`a_lambda_parameter_is_cast_from_the_erased_descriptor`. Corpus:
`corpus/features/LambdaObjects.java` (a program interface `IntOp` with lambdas and a named
implementor, an array of them applied in a loop, a lambda returned from a static method, lambdas
of a generic program interface `Transform<A, B>` erased), `corpus/features/MethodReferences.java`
(static, unbound, constructor and array-constructor references to program methods, `String::length`,
`Integer::parseInt`, `String::toUpperCase`, an overloaded target resolved by the descriptor).
Unblocks: nothing alone; the base for all below.

**Closure forms.** `analysis/closures.rs` and `codegen/closures.rs`: closure-typed interfaces
(4.2), the parameter, local and stored forms (4.3), aliases, calls, `Object` variants, bridges
between erased and typed closure types (4.4), default and static methods of closure-typed
interfaces as extension traits and free functions (4.6, on program interfaces first), the
`&mut dyn FnMut` fallback. Shape tests: `a_called_only_parameter_is_impl_fn_mut`,
`a_stored_lambda_is_an_rc_dyn_fn`, `an_interface_with_a_class_implementor_stays_an_enum`,
`a_default_method_on_a_closure_type_is_an_extension_trait`, `a_typed_closure_bridges_to_an_erased_one`.
Corpus: `corpus/features/ClosureParameters.java` (higher-order static methods taking program
functional interfaces, nested calls, a recursive method passing its function on, a method
wrapping its function into a new lambda and recursing), `corpus/features/StoredClosures.java`
(a lambda in a field called later, a list of lambdas run in order, a lambda returned and called
twice, `==` on a functional value with itself, a `null` functional field),
`corpus/features/FunctionalDefaults.java` (a program interface with default methods returning
lambdas, chained, and a static factory method), `corpus/programs/RuleEngine.java` (predicates
and actions as lambdas over a record-free `Order` class, composed with `and`/`or`-like defaults).
Unblocks: `LambdaFunctions` in single-file mode except `java.util.function`'s defaults.

**Lambda captures.** After the captures milestone merges: locals, parameters, `this` with the
borrowed and handle forms (1.3, 4.3), `Outer.this`, arrays, captures of captures (a lambda in
an anonymous class in a lambda), bound method references with any receiver, the prelude block,
`Send`-ready `Arc` forms behind the multi-threading predicate, self-reference through fields
(4.8). Shape tests: `a_lambda_capturing_this_in_a_borrowed_parameter_borrows_self`,
`a_stored_lambda_capturing_this_takes_the_handle`, `captures_are_cloned_in_a_prelude`,
`a_bound_reference_evaluates_its_receiver_once`. Corpus:
`corpus/features/LambdaCaptures.java` (captured ints, strings, objects, arrays mutated through
the lambda, a captured loop variable copy per iteration, a capture of a capture),
`corpus/features/LambdaCapturesThis.java` (an instance method storing a lambda over its fields,
a borrowed lambda updating a field inside a call on another field, a lambda calling back into
the object that invokes it: the re-entrancy case), `corpus/features/BoundReferences.java`
(`this::m`, `super::m`, `obj::m` with a receiver that changes afterwards, `System.out::println`),
`corpus/programs/Spreadsheet.java` (cells with formula lambdas capturing other cells, evaluated
in dependency order). Unblocks: `LambdaFunctions`, `PriorityScheduler` (the `Comparator` lambda),
`EventListeners` (with anonymous classes and for-each), `TryWithResources`'s lambda
`AutoCloseable` (with the exceptions work).

**JDK functional interfaces end to end.** Whole-program mode with the JDK sources:
`java.util.function.*` with their defaults (`andThen`, `compose`, `negate`, `and`, `or`,
`identity`), `Comparator`'s statics and defaults over the closure form and the enum form
(`comparing`, `comparingInt`, `thenComparing`, `reversed`, `naturalOrder`, `nullsFirst`),
`Iterable.forEach`, `Map.forEach`/`computeIfAbsent`/`merge`/`replaceAll`, `List.removeIf`/
`replaceAll`/`sort`, `Objects.requireNonNull(T, Supplier)`, `Preconditions` exception factories,
exceptions in lambdas (4.7), `Runnable` to `Thread` once the multi-threading "threads by
subclassing" milestone exists. Shape tests: `comparator_comparing_is_a_closure_returning_function`,
`a_throwing_lambda_group_returns_result`. Corpus: `corpus/projects/comparators` (sorting with
`comparing` chains, `reversed`, `nullsFirst`, an anonymous comparator and a lambda comparator in
one program), `corpus/projects/functional-chains` (`Function`/`Predicate` composition, `Map` and
`List` lambda methods), `corpus/features/ThrowingLambdas.java` (a user `throws` interface, a
lambda throwing an unchecked exception caught by the caller, a lambda that throws nothing in a
group where another does), `corpus/threads/RunnableThreads.java` (two `Runnable` lambdas and an
anonymous `Runnable` started as threads and joined). Unblocks: `ComparatorSorting`,
`LambdaFunctions` (whole-program), `IntegerKeyedGroups` (`computeIfAbsent`), `WordFrequency`
(`merge`), `StreamPipeline` after `docs/lists-and-streams.md`, and the executors milestone of
`docs/multi-threading.md`.

## 8. Risks and open questions

- **`RefCell` panics from re-entrant callbacks.** A lambda that calls back into the object
  running it is ordinary Java. Rule 1.3 covers it, at the price of `&self` methods becoming
  handle methods wherever any lambda of a group may reach the class. Recommendation: accept,
  count the methods that lost `&self` because of closures in the report, and let the
  thorough-mode points-to analysis win them back per site.
- **Monomorphization of `impl FnMut` parameters.** A method that wraps its function parameter
  into a new lambda and recurses would instantiate itself endlessly. Recommendation: the
  analysis detects a parameter flowing into a lambda that flows back into the same method and
  uses `&mut dyn FnMut` there; a shape test pins it.
- **Lambda identity.** HotSpot returns one object for repeated evaluations of a non-capturing
  lambda; the JLS does not promise it. Recommendation: `Rc::ptr_eq` identity, documented; the
  fuzzer never compares two evaluations of one lambda expression.
- **Unified signatures over a group make lambdas pessimistic.** One lambda returning `null`
  makes every lambda of the interface return `Option`; one throwing makes every call `?`.
  Recommendation: accept (it is the object model's rule for overrides); the per-value closure
  typing of 4.4 already splits groups by instantiation, which is where it matters most.
- **`Object` conversions in erased JDK code.** `Comparator.comparing` over erased generics
  passes `Object`s and converts at every step. Recommendation: accept as the faithful form; the
  object model's milestone 6 removes it as the collections become real generics; measure with
  the benchmark `SortPeople` (a million `comparing`-sorted objects) before and after.
- **Captures in constructors.** Rejected in the first version (1.5). The JDK's collections
  create their inner objects lazily, so no target is blocked; a program registering `this` as a
  listener in its constructor is. Recommendation: two-phase construction after the first
  milestones; never `Weak`.
- **Anonymous classes with fields of functional interfaces** stay structs (3.3), so a program
  mixing them with lambdas makes the interface an enum. Recommendation: accept; the closure form
  with a `RefCell` inside is a later refinement if the report shows such programs.
- **Marker interfaces.** `Serializable` appears in intersection casts throughout
  `java.util.Comparator` and as a supertype of every collection. Recommendation: drop
  `Serializable`, `Cloneable` and `RandomAccess` in lowering wherever they appear (the targets
  README counts seven programs blocked by `java.io.Serializable` today).
- **Open: `Arc<dyn Fn>` versus `Rc<dyn Fn>` per value in a multi-threaded program.** The
  type-based escape analysis decides per lambda class, so a lambda class with one site handed to
  a thread and another kept local is `Arc` at both. Recommendation: accept, as for every class;
  per-site with the points-to analysis.
- **Open: should closure parameters be `impl Fn` where no lambda mutates?** `impl FnMut` is the
  general bound and costs nothing at a call. Recommendation: `impl FnMut` always; clippy does
  not object, and one form keeps the generator simple.
- **Open: `java::iterate` versus a generated `while`.** The adapter adds a runtime trait and
  an `impl` per iterator class. Recommendation: keep it; `for x in java::iterate(it)` is what a
  reader expects, and the `while` form stays for the `Result` case.
- **Open: an enum named `Iterator` shadows the prelude trait** in its module (the object model
  names enums after the Java type). Legal Rust, and generated code never names the trait
  (`java::iterate` returns `impl Iterator` inside the runtime). Recommendation: accept the
  shadowing; the generator writes `std::iter::Iterator` if it ever needs the trait.

## 9. Deviations

Decisions that change during the implementation are recorded here, milestone by milestone.

### Captures and local classes

- **Desugared as javac does, so no new expression kinds.** Lowering turns each inner, local and
  anonymous class into an ordinary class: a synthetic field `outer` (first in the struct) for
  the enclosing instance and a field per captured local, constructor parameters for them (the
  enclosing instance before the declared ones, the captured locals after them, as javac orders
  them), assignments of the fields at the start of every constructor that does not call
  `this(..)`, and the arguments in every `new`, `this(..)` and `super(..)`. `Outer.this` and an
  unqualified member of an enclosing class become a chain of `outer` field reads
  (`this.outer.outer`), starting at the `outer` parameter inside a constructor; a captured local
  reads its field (its parameter, inside a constructor). So `ExprKind::New` keeps its shape and
  there is no `ExprKind::OuterThis`: the analyses and the generator see fields, parameters and
  arguments they already handle, and the this-as-value facts of 1.3 fall out of the existing
  ones (a method that creates a local class reading the object's fields passes `this` as a
  value, which already gives it the handle form). `analysis/captures.rs` and
  `codegen/captures.rs` are therefore not needed; `lower/captures.rs` computes the captures on
  javac's tree before lowering, and `lower/inner.rs` declares, names and desugars the classes.
- **The AST additions differ in detail.** `TypeDecl` has `outer: Option<usize>` and
  `captures: Vec<Capture>` as planned, but `Capture` also names its field (`field`, which gets a
  `_` appended where the class or a superclass already has a field of that name), and
  `Origin` is `Member`, `Local { method, binary }` or `Anonymous { method, binary }` (binary:
  javac's binary name, which `getName` and the default `toString` print). The lambdas work adds
  its own variant. The captures are informational for later analyses (threads); the struct
  fields are already in `fields`.
- **Field initializers move into the constructors** of a class that holds an enclosing instance
  or captures, after the synthetic assignments and before the initializer blocks (javac's
  order), since they may read what those set.
- **The synthetic fields are set after `super(..)`, not before it** as javac does: the
  superclass's constructor builds the struct's `base`. A superclass constructor that calls a
  method overridden in the nested class, which reads a capture, would see it unset; no program
  in the corpus or targets does that.
- **`this` captured in a constructor is not rejected** (1.5): a constructor that uses `this` as
  a value already builds the object behind its handle first (`field_inits` and then the body on
  the handle), so `new Inner()` in a constructor or a field initializer works.
- **JavaTree names local and anonymous classes by their binary names** (`Main$1Labeler`,
  `Main$1`) in types, symbol types and owners, where javac's `toString` gives the simple name or
  `<anonymous Runnable>`, which is ambiguous between two methods. Enum constant bodies keep
  javac's names, which `lower/constant_bodies.rs` relies on. A separate commit; the lambdas
  implementer owns the file and is told.
- **Reachability declares local and anonymous classes** (with the class whose member declares
  them as their outer class), so that their members are in `Reach::methods` and are lowered in
  whole-program mode. Their code is not scanned with the code that declares them: a constructor
  is reached by its `new`, an instance method by a call or by dispatch once the class is
  instantiated, as for any other class (the streams work; a stream operation's anonymous `Sink`
  and `StatefulOp` no longer reach their parallel overrides).
- **Local interfaces, enums and records in a file with a package** are hoisted like the others
  (`lower/local_types.rs`), named by javac's binary name (`Main$1Color`); they capture nothing.
  **Member classes of local and anonymous classes** are named after their outer class and their
  own name (`Main$1Counter.Tally`, from javac's `Main$1Counter$Tally`), and capture what they read, like local classes
  (`Lowering::member_outers`). **`outer.super(..)`** passes the qualifier as the superclass's
  enclosing object (`hierarchy.rs` `super_init`), and **`Outer.super.m()`** calls the superclass's
  method on the enclosing object (`hierarchy.rs` `super_receiver`).
- **Enum constant bodies may declare fields** (`lower/constant_bodies.rs`): each becomes a field
  of the enum, whose value for a constant is its body's initializer, or the zero of its type for
  the constants whose body does not declare it. Enum values are recomputed on each read, so the
  initializer must be pure as the enum's are (a constant), and the fields are of the types
  `int`, `long`, `double`, `float`, `char`, `boolean` and `String`. Still rejected in a body:
  blocks, member classes, static fields, fields of other types, and initializers that are not
  pure, since a side effect would repeat on each read.
- **Still rejected:** a nested class extending one nested in another class (the two `outer`
  fields would differ); a field named `outer` in a nested class with an enclosing instance; an
  enclosing instance needed in static code (which javac rejects as well); and `Outer.this` or
  `Outer.super` from an inner class that itself extends `Outer` (`enclosing_instance` stops at
  the first subclass of `Outer`, which is then `this`, not the enclosing object).

### Inner classes

- **Inner classes come with the captures commit**, as the same desugaring covers them; this
  milestone adds their corpus, shape tests and survey rows. An inner class extending a static
  nested class keeps `outer` after `base`; inner subclasses of an inner class share the
  superclass's `outer`.
- **`is_inner` is renamed `has_subclasses`**, not `has_fields_struct`: it holds for every class
  with subclasses, and `fields_structs` picks those of them that get a fields struct.
- **An inner object kept in a field of its outer object is a reference cycle** (the outer
  object holds the inner one, which holds `outer`), which `Rc` never frees where Java's
  collector would; a `HashMap` that caches its `keySet()` leaks with its view. Programs print
  the same; memory is the cost, as with any other cycle of objects in the translation.

### Anonymous classes

- **Anonymous classes come with the captures commit too**; this milestone adds their corpus,
  shape tests and survey row, and fixes found on the way: a static field of an interface type
  had its initializer unconverted (no variant), a method of an immutable class took `&mut self`
  when another method of its override group changes its object, and an anonymous subclass of a
  local class was named after the local class's binary name (`1CounterInChained`).
- **The name of an anonymous class has no enclosing class**: `<package><Supertype>In<Place>`
  (`OrderInMain`, `TaskInInit`) as 2.5 says, numbered from 2 on a clash.
- **An anonymous class extending a class or implementing an interface of the JDK** (`new
  Runnable() { .. }`, `new Comparator<..>() { .. }`) needs those types as program types: the
  lambdas work's runtime interfaces in single-file mode, the JDK's sources in whole-program
  mode. Without them it is rejected as "implementing `java.lang.Runnable`".

### For-each over `Iterable`

- **Lowered to the loop Java runs, in one form.** `for (T x : c) body` on an object of the
  program becomes `{ it = c.iterator(); while (it.hasNext()) { T x = (T) it.next(); body } }`
  in lowering (`lower/foreach.rs`), with ordinary calls of the program's (or the JDK's)
  methods, resolved as javac would (the receiver's class, then its supertypes). The generator
  then writes `while it.has_next() { let x = it.next(); .. }`, propagating what the calls throw
  like any other call. The idiomatic `for x in java::iterate(..)` form of section 5, with
  `impl java::Iterating` items and `crates/runtime/src/iterate.rs`, is not written:
  the `while` form has Java's call sequence by construction, including after `break` and when
  the body changes the collection, and needs no new statement kind in the analyses. It can be
  added later as a generator pattern over this shape where neither call throws.
- **Reachability adds `Iterable.iterator`, `Iterator.hasNext` and `Iterator.next`** at every
  for-each over a non-array, since javac's tree does not show those calls; their overriders in
  live classes follow as for any call.
- **A label on the loop goes on the `while`**, inside the block that declares the iterator.
- **The JDK's packages as modules:** a whole program with JDK classes wrote `mod java`, which
  clashed with the runtime's alias `java`; package segments `java`, `std`, `core`, `alloc` and
  the path keywords now take a `_` (`crate::java_::util::Iterator`).
- **Single-file mode takes `Iterable` and `Iterator` from the lambdas work's
  `java/Functional.java`**, which declares their abstract methods only. So a call of
  `Iterator.remove` (a default method of the JDK's) on an `Iterator` is rejected in a file as
  "the default method java.util.Iterator.remove", where before the merge it reached the
  analyses unresolved; `corpus/features/IteratorRemove.java` declares its own
  `RemovingIterator` for it. With the JDK's sources the JDK's `Iterator` is translated, default
  methods included.
- **No corpus project over the JDK's collections**: the corpus test runs without the JDK's
  sources, so the two for-each programs were also checked by hand in whole-program mode with
  `--library` and the JDK's `Iterator` (same output as Java). A for-each over the JDK's own
  collections still stops before it, at what those classes use (`ArraysSupport`'s fields,
  `new Object()` in `TreeMap`).
### Lambdas and method references

- **Captures came before the closure forms.** The captures landed while the object form of
  lambdas was done, and lambdas as objects take them with no new machinery (below), so the
  lambda captures milestone was done second and the closure forms milestone follows it.
- **A lambda lowers to `new` of its class, converted to the interface**, not to a new
  `ExprKind::Lambda`: `Cast(Interface, New { class, args, ctor })`. The class is the synthetic
  concrete class of 4.1, declared with the members of the class whose code holds the lambda. So
  the object model's variants, dispatch, bridges and the analyses take it with no new case.
- **A lambda's class is a nested class** (`NestKind::Lambda`, `Origin::Lambda`), declared in the
  class and method whose code holds it (an enclosing lambda's class and method, for a lambda in
  a lambda), so `lower/captures.rs` finds what it captures as for a local class: the walk of the
  enclosing code takes a lambda as an object of a class declared there, and the walk of the
  lambda's own code is its body. Captured locals are fields set by the constructor from
  trailing parameters that the creating expression passes (`OpLambdaInMain::new(k, &name)`),
  and `this`, or a field or method of the enclosing class named without it, reads the field
  `outer` (`self.outer.borrow().base`); in a lambda in a lambda, `this` is the outermost class's
  object, reached through both `outer` fields. There is no prelude block of clones: the
  arguments of the `new` are the clones. `analysis/closures.rs` and `codegen/closures.rs` stay
  for the closure forms.
- **The class is named `<Interface>LambdaIn<Method>`** (`IntOpLambdaInMain`, numbered
  `IntOpLambdaInMain2` in the order of the source), in the package of the enclosing top-level
  class; a field initializer or block is named after the class, a constructor too. The variant of
  the interface's enum repeats it (`AIntOp::IntOpLambdaInMain`). `getClass().getName()` gives
  `Main$$Lambda`, the JVM's name without its varying suffix.
- **The method takes javac's substituted descriptor** (`descriptor.substituted`, which
  `JavaTree` prints with the declared and erased types and the referenced method's type), so a
  lambda of `Function<Integer, Integer>` is `apply_integer(i32) -> i32` with the bridge
  `apply(Object)`, exactly as a class implementing the interface gets.
- **A method reference's body is a call that lowering builds as javac tree JSON**
  (`METHOD_INVOCATION`, `NEW_CLASS` or `NEW_ARRAY` on parameters `a`, `b`, .., skipping names
  that the receiver uses), made when the class is declared, and lowered like any call. So
  `String::length`, `Integer::parseInt`, `Math::max` and `System.out::println` get the same
  library mappings as written calls, javac's referent symbol picks the overload, and the capture
  walk sees the receiver in that call.
- **A bound method reference's receiver must be a local, `this`, a string literal,
  `System.out`/`System.err` or a final field** (read through `this` or a class), whose value
  where the reference is evaluated is the value at every call. Any other receiver
  (`new StringBuilder()::reverse`, `a.b()::m`, a field that is not final) is rejected, and
  `--survey` counts those as "computed method receivers" (lambdas and method references are no
  longer features of the survey). javac also checks the receiver for null where the reference
  is; the translation checks it at the call.
- **`super` in a lambda (`super.m()`, `super::m`) is rejected.**
- **The JDK's functional interfaces in single-file mode come from a template**
  (`crates/translator/java/Functional.java`, compiled by `build.rs` like `Standard.java`): the
  abstract methods of `Runnable`, `Callable`, `Comparator`, `Iterable`, `Iterator` and the
  `java.util.function` interfaces, declared for the program when it mentions them. Their default
  and static methods are not in it: written by hand they would be a second copy of the JDK's
  code, so a call of one in single-file mode is rejected with a message that points to
  whole-program mode (`--main` and `--library`).
- **The JDK functional interfaces milestone is whole-program only**, and came before the closure
  forms. With the JDK's sources the interfaces are the JDK's, and their default and static
  methods, with the lambdas in them that capture `this` or parameters
  (`ComparatorLambdaInThenComparing { outer, other }`), are translated like the program's.
  What it took was outside the lambda code:
  - `new Object()` as an object of a class of its own (`PlainObject`, its own commit);
  - the methods of `java.util.Objects` other than the hand-written `equals`, `hashCode`, `hash`,
    `isNull` and `nonNull` go to the translated `Objects.java` (`requireNonNull`,
    `requireNonNullElseGet`, ...);
  - `java.lang.Comparable` is one of the runtime's classes in whole-program mode. Its source made
    a second `Comparable` enum that strings and boxes were not in, so `Comparator.comparing` on a
    string key failed its cast;
  - an enum may implement an interface's own `equals` or `hashCode` (`Comparator.equals`) with
    the one every enum has;
  - the analysis of failures that a `catch` sees (`analysis/exceptions.rs`) knows more values not
    null: an `instanceof` test proves its variable not null and its cast safe, `!` is followed,
    `if (c) return;` (or `throw`, `break`, `continue`) narrows the statements after it, and a
    local initialized from such a value is not null (`corpus/features/GuardedCasts.java`). The
    `catch (ClassCastException | NullPointerException unused)` of `AbstractMap.equals` sees every
    `equals` that a map's values may run.

  The programs are in `corpus/whole-program/`, run by `tests/whole_program.rs` when
  `JAVA_BASE_SOURCES` names the JDK's sources. Still blocked: `Comparator.reversed()` and
  `reverseOrder()` reach `java.util.Collections`, whose empty collections' iterators are eagerly
  initialized statics that the nullness analysis takes as possibly null, so `AbstractList.equals`
  may fail where `AbstractMap.equals` catches it; `NullPointerException.getMessage()`.
- **Marker interfaces are dropped** where they are named as supertypes and from intersection
  casts (`(Op & Serializable) a -> ..`), in `JavaTree` (`javaType` of the cast) and in
  `hierarchy::interfaces`.
- Fixed on the way, in shared files: a string method taking an `Integer` (`"ab".repeat(n)`)
  passed a reference; a `StringBuilder` converted to `Object` panicked in the generator instead
  of being rejected; and a call of an interface method returning an array, on an interface with
  one implementor, called the interface's `make`.
- **Still rejected:** the receivers and `super` above; a functional interface whose descriptor
  has an array of a type variable (`IntFunction<int[]>`, as any generic method, 4.4); a
  `StringBuilder` or array as the value of a generic functional interface
  (`Supplier<StringBuilder>`: it would be an `Object`); a lambda converted to a JDK interface
  outside the template in single-file mode; creating a local class declared in a lambda where
  a local hides one of its captures.

### Closure forms

- **The first closure types are a subset of 4.2**: an interface is one when it has one abstract
  method (besides `Object`'s, which `Comparator` declares again), or extends exactly one
  interface and declares none, every class below it is a lambda's that implements one
  interface, every interface above and below it is a closure type too, every `new` of such a
  class is converted to the interface right away, and no value of it is converted to another
  type than an interface above it (`Object`, printing, a collection), compared with `==`,
  tested with `instanceof`, passed to a builtin or called with anything but its method and its
  default methods; and each lambda's method uses `this` only to read its capture fields.
  Whatever rules out one interface rules out those above and below it. Interfaces seen as
  objects (condition 3 of 4.2 as written, and identity) stay enums for now.
- **An interface that extends another names its closures**: `type UnaryOperator = Function;`, so
  a `UnaryOperator` is passed where a `Function` is taken as it is, and calls the default
  methods of `FunctionMethods`.
- **A generic interface's closure is erased, per interface, not per instantiation** (4.4 asks
  for the substituted types at each declaration site): `Function<Integer, Integer>` is the
  alias `type Function = Rc<dyn Fn(Object) -> Object>;`, and a call boxes the arguments and
  casts the result as the enum's dispatcher did. The lambda's method, which javac typed with the
  substituted types, sits behind a bridge (4.1); the closure's body is the bridge's casts of the
  arguments bound to the method's parameters, then the method's body, with the bridge's
  conversion around its value:
  `Rc::new(|t: Object| -> Object { let x: i32 = java::cast(t.as_integer(), "java.lang.Integer"); Object::Integer(x.wrapping_mul(x)) }) as Function`.
  A result that needs no conversion (`Predicate.test`, `Consumer.accept`) takes the method's
  body as the closure's whole body. A method that returns before its end, or throws, and whose
  result is converted, is a closure called at once inside the outer one,
  `Object::Integer((|x: i32| -> i32 { if x > k { return x.wrapping_sub(k); } .. })(java::cast(..)))`,
  since its `return`s would leave the outer closure with the unconverted value.
- **Only the stored form, `Rc<dyn Fn(A) -> R>`,** for every place: locals, parameters (`&Op`, as
  the borrowing analysis already lends objects), fields, statics, elements and returns. The
  parameter form `impl FnMut` and the local form `let f = |x| ..` of 4.3 are later refinements,
  as is a type alias per signature instead of per interface (the alias is named after the
  interface: `type ClosureFormsIntOp = Rc<dyn Fn(i32, i32) -> i32>;`). The argument and result types are
  those of the interface's method as its dispatcher would take them (`Fn(&str) -> i32` for a
  borrowed string parameter, `Result<_, Throwable>` when the method throws).
- **A lambda is written where it is created** (`codegen/closures.rs`): the lambda class's method
  is generated inside the creating expression with its own names and facts (the generator's
  method state is set aside and restored), and its capture fields are locals bound before the
  closure and moved in (`field_locals`), so `this.k` is `k` and `this.outer` is `outer`. A
  local passed as it is moves in under its own name; the closure owns what the class's
  constructor would have borrowed. A closure with no captures has no `move`, and a one-line
  body stays on the closure's line. The value is `Rc::new(closure) as Op`, so that every place
  gets the alias's type without an annotation; the captures are bound in a block around it,
  `{ let outer = self.clone(); Rc::new(move |x| ..) as Op }`, except where the closure is a
  function's final value: there they are statements of the function, and the result type
  converts `Rc::new(move |x| ..)` without `as`.
- **Default methods are an extension trait** (4.6): `trait PredicateMethods { fn negate(&self)
  -> Predicate; .. }` beside the alias and `impl PredicateMethods for Predicate`, with only the
  default methods the program calls. `self` is `&Rc<dyn Fn ..>`: `test(x)` is `self(x)`, and a
  lambda that captures `this` gets `let outer = self.clone();`. A call is method syntax,
  `empty.negate().and(longer)`, and a module that calls the default methods of a closure type
  of another module imports its trait (`use crate::java_::util::function::PredicateMethods;`).
  Two closure types with the same Rust type and a default method of the same name would make
  such a call ambiguous; no two of `java.util.function` do, and no corpus program has such a
  pair.
- **A closure type that threads may share** (`Facts::thread_shared` holds it or an interface
  above or below it, which share its closures) is `Arc<dyn Fn(..) -> R + Send + Sync>` and its
  closures `Arc::new(..)`, the handle chosen in one place (`closure_handle`); its captures are of
  shared types by the same walk. Others keep `Rc`, also in a threaded program
  (`docs/multi-threading.md`). Corpus: `threads/ClosuresAcrossThreads.java`,
  `threads/ClosureMethodsAcrossThreads.java` (default methods on shared closures).
- **A call clones a closure out of a `RefCell`** before running it,
  `({ let f = a.borrow().op.clone(); f })(3)`, since the closure may reach the object again; a
  local, a capture or a field of a plain object is called in place (`op(x)`, `(self.op)(x)`).
- **Corpus**: `features/ClosureDefaultMethods.java` (default methods that create lambdas over
  `this`, call each other and call the closure, and a static method);
  `whole-program/FunctionChains.java` (`Predicate`, `IntPredicate` and `BiFunction` from the
  JDK's sources with their default methods, `Function` with `UnaryOperator`);
  `features/GenericClosures.java` (the generic interfaces of `java.util.function` and the
  program's own: an early return, a lambda that throws behind a bridge, a single lambda of an
  interface passed as an argument, a generic method calling its closure, a `UnaryOperator`
  passed as a `Function`); `features/ClosureForms.java` (captures, `this` through `outer`, a
  field and a final field of closures, an array of them, a closure returned from a method, a constructor
  reference, a closure that throws a checked exception, a closure in a closure).
