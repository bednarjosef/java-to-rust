# The object model: inheritance, the Object root, interfaces, generics, instanceof and casts

How the translator represents the program's class trees, interfaces, `java.lang.Object`,
generics, `instanceof` and casts in Rust, why, and where it lives in the code. The program is a
closed world: every class that can exist at run time is in the translation, so a type's
possible classes are known and dispatch can be a `match`.

## 0. The decisions in one page

1. **A concrete class is a struct; inherited fields are embedded.** A class with subclasses has
   a *fields struct*, `<Name>Fields`, with the fields it declares and its superclass's fields
   struct as `base`. A subclass's struct embeds it as `base` (`struct Circle { base:
   BaseFields, r: f64 }`). Embedding rather than flattening because `super(...)` becomes the
   `base:` entry of the constructor's struct literal, a method that only touches inherited
   fields is emitted once on the fields struct, and an enum needs one `base()` accessor rather
   than one per field.
2. **A type that objects of several concrete classes can be is an eight-byte reference named
   after the Java type**, a `java::Thin` holding the class's number and the object's handle,
   with a constructor per class whose objects become values of the type (`Base::Circle(o)`)
   and a *view*: an enum with one variant per such class that lends the class's object
   (`Arc<RefCell<C>>`, or `Arc<C>` when immutable). The code calls such a type and its view
   "the enum" of the type. A type with one such class is that class's object, with no enum.
   Enums are flat: every variant is a concrete class, never another enum, so a call through an
   interface over several class trees is one `match`.
3. **Dispatch is a `match` in a method of the enum** (a *dispatcher*). A body that a class
   inherits is emitted once: on the fields struct when it uses `this` only for fields (a
   *field-level* method), otherwise on the enum of the declaring type or in the one concrete
   class's `impl` (an *object-level* method).
4. **Conversions are explicit in the tree.** Lowering inserts a `Cast` at every implicit
   reference conversion, so the analyses see every upcast, and the generator writes
   `Base::Circle(c)` (class to enum), `b.to_shape()` (enum to enum) or a checked downcast.
5. **`instanceof` is a test of the class's number** (`matches!(s.0.tag(), 2 | 5)`), and a
   pattern variable is bound with a let chain, `if let`, `let .. else` or `is_some_and`.
6. **`Object` is an enum too**, over the program's classes whose objects become `Object`s and
   over strings, boxes, enums, arrays, builders and class values as runtime variants. Its
   `equals`, `hashCode` and `toString` dispatch to the class's own or to Java's defaults.
7. **Identity is `PartialEq` on the enums**: pointer equality of the objects of one class,
   `false` across classes.
8. **Representation (shared, immutable, owned) stays per concrete class**, with rules for
   inheritance: a class that has subclasses, is converted to a supertype or runs an inherited
   object-level method is never owned; the concrete classes of one class tree (a *family*) are
   immutable together or not at all.
9. **Generics: erased first, real where proven.** Every generic class and method is erased as
   javac erases it, with javac's checked casts at the use sites. Simple containers whose type
   variables' values are only stored, returned and passed on are real Rust generics instead.
   The erased form is always the fallback, never a rejection.
10. **Exceptions are a class tree like any other** under `Throwable` (see docs/exceptions.md).

## 1. The representation

### 1.1 Vocabulary and where it lives

The tree's hierarchy table is `ast/hierarchy.rs` (`Hierarchy`): direct subtypes, overriders,
superclasses, interfaces, which types are concrete, and the queries `ancestors`, `concretes`,
`group` (an override group: a method, what it overrides and what overrides those),
`targets` (the implementations a call may run given the receiver's static type),
`implementation` (what an object of class `c` runs for method `m`), `field_owner` and `family`
(the concrete classes of one class tree). Lowering fills it from javac's `extends`,
`implements` and each method symbol's `overrides` list (`lower/hierarchy.rs`).

`analysis/hierarchy.rs` decides where things live: `fields_structs` (which classes get a fields
struct), `field_level` (which methods live on one), `flows` (which classes' objects become
values of which types), `Rep` (a type's values are one class's object, `Rep::Object`, or an
enum, `Rep::Enum`) and `Home` (where a method's body is emitted: a class's `impl`, a fields
struct, an enum, a free function, or nowhere for an abstract method).

`codegen/dispatch.rs` writes the enums. Each item of an enum (`base()`, `base_mut()`, a
dispatcher, a body on the enum, a `to_x()` or `as_x()` conversion, `Display`, `java_string()`,
`try_text()` and `nullable_text()` (the string its `toString` returns), and the methods
that answer for the object's class: `runtime_class()`, `runtime_class_name()`,
`runtime_type_names()` and `object_identity_hash()`) is a `Need`, and is written only when some
code uses it: the generator writes the program again with the items asked for until no new one
is asked for (two or three passes), so an enum has no dead code. A `match` over every variant is
written once, in such a method, and not at each use: `Object`'s enum has a variant for every
class whose objects the program creates (javac's has thousands).

Terms:

- **leaf class**: a concrete class with no subclasses in the program.
- **fields struct**: for a class with subclasses that declares instance fields, has objects of
  its own, embeds an ancestor's fields struct, or has a constructor that does something.
  `struct <Name>Fields { base: <Super>Fields, <own fields> }`. A class with subclasses that is
  itself instantiated has a variant holding its fields struct. A superclass without a fields
  struct has a constructor that does nothing; only the effects of the arguments of `super(...)`
  are written, as statements.
- **variants of a type**: the classes whose objects become values of it (`hierarchy::flows`): a
  `new`, an upcast from a type whose values they are, a call of a method the type declares on a
  receiver of a subtype, `super.m()`, a pattern binding, and running a method of the type that
  is not field-level (its `this` is a value of the type). A class whose subclasses are only ever
  used through an interface therefore stays a plain struct. Owned classes are never variants.
- **family**: the concrete classes of one class tree. They decide immutability together (1.8).
- **field-level method**: an instance method of a class with a fields struct that uses `this`
  only to read and write the fields of its class and ancestors and to call field-level methods
  that nothing overrides (also through `super`). It never uses `this` as a value and never makes
  a virtual call. It borrows the fields struct for the whole call, so nothing it runs may reach
  an object of its class tree through another handle, unless the tree is immutable. It is
  emitted as `impl DFields { fn m(&self | &mut self, ..) }`.
- **object-level method**: every other instance method of a type that is not a leaf class:
  interface default methods, methods that make virtual calls or use `this` as a value. Emitted
  on the enum (`self: &D`, `ThisForm::Enum`) or, when the type has one concrete class, in that
  class's `impl`.

When in doubt a method is object-level: that is always correct, field-level is an
optimization.

Naming:

- Structs and enums use the class's struct name (`Outer.Inner` is `OuterInner`); the fields
  struct appends `Fields`.
- A variant is named after its class's simple name (`Shape::Circle`, and `ShapeView::Circle`
  in the view), with the outer classes' names when two variants of one enum would share one.
  Runtime variants are `String`, `Integer`, ..., `IntArray`, `StringBuilder` and so on. The
  view is `<Name>View`, or `<Name>Variants` when the program has a class of the first name.
- Accessors are `base()`, `base_mut()` and `view()`; conversions `to_<type>()` and
  `as_<type>()`. A Java method whose name would be `base`, `base_mut`, `view`, `clone` or one of
  the class methods above gets a suffix (`base_2`, `clone_2`).
- An inherited body that a subclass overrides is emitted under `<m>_in_<declaring class>`
  (`to_string_in_shape`, `withdraw_in_account`) so that `super` calls reach it; a method that
  nothing below overrides keeps its own name.
- Instance methods are named once per override group within the class trees the group reaches,
  so every implementation has the name its callers use; overload suffixes count the groups of a
  name (`codegen/names.rs`).
- A field named `base` in a class that embeds a `base` is renamed `base_`.
- A program class named like a standard class or interface the translator supplies is
  rejected ("a class named like the standard class ...").

### 1.2 A class tree with an abstract base and overrides

```java
interface Shape { double area(); default String describe() { return "shape " + area(); } }
abstract class Base implements Shape {
    protected String name; int uses;
    Base(String name) { this.name = name; }
    String getName() { uses++; return name; }                // field-level
    public String toString() { return name + ":" + area(); } // object-level: calls area()
}
class Circle extends Base {
    final double r;
    Circle(double r) { super("circle"); this.r = r; }
    public double area() { return 3.0 * r * r; }
}
class Rect extends Base {
    double w, h;
    Rect(double w, double h) { super("rect"); this.w = w; this.h = h; }
    public double area() { return w * h; }
    public String describe() { return "rect " + w + "x" + h; }
}
```

The shape of the Rust (simplified):

```rust
struct BaseFields { name: java::JString, uses: i32 }
impl BaseFields {
    fn new(name: &java::Wtf8) -> BaseFields { BaseFields { name: name.to_owned(), uses: 0 } }
    fn get_name(&mut self) -> java::JString { self.uses = self.uses.wrapping_add(1); self.name.clone() }
}

struct Circle { base: BaseFields, r: f64 }
impl Circle {
    fn new(r: f64) -> Circle { Circle { base: BaseFields::new(java::wtf8("circle")), r } }
    fn area(&self) -> f64 { 3.0 * self.r * self.r }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
struct Base(java::Thin);
enum BaseView<'a> { Circle(&'a Arc<RefCell<Circle>>), Rect(&'a Arc<RefCell<Rect>>) }
impl Base {
    fn Circle(o: Arc<RefCell<Circle>>) -> Base { Base(java::Thin::pointer(2, o)) }
    fn Rect(o: Arc<RefCell<Rect>>) -> Base { Base(java::Thin::pointer(3, o)) }
    fn view(&self) -> BaseView<'_> {
        match self.0.tag() {
            2 => BaseView::Circle(unsafe { self.0.as_pointer() }),
            3 => BaseView::Rect(unsafe { self.0.as_pointer() }),
            tag => java::bad_tag(tag),
        }
    }
}
impl Base {
    fn base(&self) -> Ref<'_, BaseFields> {
        match self.0.tag() {
            2 => { let o: &Arc<RefCell<Circle>> = unsafe { self.0.as_pointer() }; Ref::map(o.borrow(), |o| &o.base) }
            3 => { let o: &Arc<RefCell<Rect>> = unsafe { self.0.as_pointer() }; Ref::map(o.borrow(), |o| &o.base) }
            tag => java::bad_tag(tag),
        }
    }
    fn area(&self) -> f64 {
        match self.0.tag() {
            2 => { let o: &Arc<RefCell<Circle>> = unsafe { self.0.as_pointer() }; o.borrow().area() }
            3 => { let o: &Arc<RefCell<Rect>> = unsafe { self.0.as_pointer() }; o.borrow().area() }
            tag => java::bad_tag(tag),
        }
    }
    fn to_string_in_base(&self) -> java::JString {
        java::concat!(self.base().name.clone(), format_args!(":{}", java::Double(self.area())))
    }
}
impl PartialEq for Base { /* Arc::ptr_eq of one class's objects, false across classes */ }
impl fmt::Display for Base { /* each variant as its class shows it */ }
```

Rules:

- A dispatcher exists on an enum for each method the program calls on a receiver of that type.
  It matches on the class number, and each arm lends the class's object where it lies, as
  `view()` would, so a call is one jump to the class's code. An arm calls what the variant's
  class runs, in the form that class uses (`o.borrow().m()`,
  `o.borrow_mut().m()`, `C::m(o, ..)`, `o.m()` for an immutable class). When every variant runs
  the same body emitted on the enum, the call names that body and no dispatcher is written. A
  dispatcher arm whose class runs a body emitted on the enum of a type below the enum's type
  wraps the object in that type's enum (`Binary::Add(o.clone()).eval()`) rather than
  converting the whole value.
- An object is held by a `java::Arc`, a handle with no count (docs/collector.md). A value of an
  enum is that handle's address with the class's number in the top 16 bits (`java::Thin`,
  `crates/runtime/src/thin.rs`); `Arc` ignores those bits when it reads its pointer, so `view()`
  lends the handle where it lies, and the value is `Copy` and eight bytes, `Option` of it too.
  Each class has one number in the whole program (`class_tag`), so `to_x()` copies the bits, and
  `as_x()` and `instanceof` test the number. What is not a program object (a string, a box, a
  Java enum constant, an array, a builder) is held in the low six bytes when it fits there and
  in a box otherwise; an array, a builder, a box with identity and a string with identity
  (`java::JStr`) are their own handles. A
  number that no class of the type has is a bug the view reports (`java::bad_tag`).
- A field read through an enum is `b.base().name` (with `.base` steps to the declaring class);
  a write is `b.base_mut().name = v`. `base_mut()` exists only for a mutable family. On a
  concrete receiver it is `c.borrow().base.name`.
- `super.m(args)` is a `MethodCall` with `is_super` set and a synthesized `this` of the
  superclass's type as receiver, so analyses that handle calls on `this` see it unchanged. It
  calls the body declared above non-virtually. `X.super.m()` is a `super` call of interface
  `X`'s default method.
- An abstract method is not emitted; its dispatcher is.
- A `toString` is a method that returns the string, as any other method is; the enum's
  `Display` (for printing) calls it per variant.
- A static method of a class or interface is a free function.

### 1.3 Places typed by the base, the interface and Object

A field, local, parameter, array element or return value of a class type with an enum holds
the enum; one with one concrete class holds that class's object. Nullness, `Option`, borrowed
parameters (`&Base` for a parameter only used through) and moves follow the same rules as for
any object type. A caller converts at the call: `total(&Base::Circle(c.clone()))`.

### 1.4 Constructors and `super(...)`

There are two constructor forms.

**The plain form** (the default). A constructor builds a plain value, a struct literal whose
first entry is the superclass's part: `Circle { base: BaseFields::new("circle"), r }`. The
`base:` value is the superclass constructor that javac resolved (or the fields struct's
`new()` for an implicit `super()`). Since `base` is the first thing Java runs and the first
entry, `codegen/owned.rs::struct_literal` keeps Java's order by binding values with effects to
locals first where the literal's field order would reorder them.

- `analysis::leading_assignments` takes the body's leading field assignments into the
  literal, skipping the `super(...)` call. A leading assignment may read fields that earlier
  ones set (`EnumMap`'s `keyUniverse = ..; vals = new Object[keyUniverse.length]`): each leading
  value goes into a local that the later ones read. Every field is then set before anything can
  read it, so the nullness analysis sees those fields as never null.
- A superclass's constructor can be a plain-value builder (`Facts::plain_constructors`) when it
  uses `this` only for the struct being built and calls on `this` only methods that borrow that
  struct, and so do its class's field initializers.
- A class whose own constructors use `this` as a value (and that has no subclasses) allocates
  the object first, `Arc::new(RefCell::new(Circle { base: .., r: 0.0 }))`, then runs the field
  initializers and the body on `this`.
- `this(...)` delegates as Java does.

**The whole-object form** (`analysis/whole_objects.rs`, run before the analyses). A
constructor of a class with subclasses that uses `this` as a value or calls a method that a
subclass may override cannot build a plain value: Java runs the subclass's override on the
object while it is being built, and with the plain form the object does not exist yet. The JDK
needs this: `AbstractPipeline`'s constructor links the previous stage to `this` and calls the
overridable `opIsStateful()`. Such a class tree is rewritten in plain Java terms so that
Java's order holds on the finished object, and the analyses and generator need nothing new:

- each constructor of a class with subclasses in the tree becomes an instance method
  `construct<Name>`, whose `super(..)` or `this(..)` is a call of the matching method on `this`,
  followed (unless it calls `this(..)`) by the class's field initializers and then its body. The
  class gets one constructor that takes only its enclosing instance and captured locals (javac
  sets them before `super(..)`) and builds the fields struct with them and every other field
  zero, null or false;
- a constructor of a leaf class that extends one of them builds its struct with that zero
  `base`, sets its enclosing instance and captured locals (javac sets them before `super(..)`
  too), calls the superclass's `construct<Name>` with the arguments of its `super(..)`, runs its
  own field initializers, then its body;
- `new C(..)` of a class with subclasses calls a static `new<Name>`, which builds the zero
  object and calls its `construct<Name>`. The enclosing instance and captured locals of the zero
  object are the constructor's parameters, or fields of them (`holder.super(..)`), that its
  `this(..)` and `super(..)` pass on.

Field initializers of these classes run after the superclass's part (in a method
`initializeFields` when several constructors run them), so a method that the superclass's
constructor calls sees the subclass's fields unset, as in Java; an object field is then an
`Option`. Every other class keeps the plain form, so code that translates with it translates
the same.

Rejected: a class with subclasses that needs the whole-object form, whose enclosing instance
or captured locals a `super(..)` computes other than as a parameter or a field of one (`new
A().super()`): "constructors of a class with subclasses that use `this` as a value or call
overridable methods, in a class with an enclosing instance or captured locals that a
constructor does not pass on as they are".

**Statements before `super(...)` or `this(...)`** (Java 25's flexible constructor bodies, used
by the JDK's `SortedOps`; `lower/prologues.rs`) are written before the object is built, so their
locals reach the call's arguments and the rest of the body. They cannot use `this`, except to
assign a field without an initializer: the value goes into a local `early$f`, and `this.f =
early$f` right after the call stores it. In the whole-object form those stores run on the zero
object before the superclass's `construct<Name>`, which may call a method that reads them, as in
Java. Before a `super()` of `Object` (which is left out), the statements are the start of the
body; a class with field initializers is rejected there, as they would run first.

**Static initialization**: a superclass is initialized before its subclass, and code in a class
or its subclass does not initialize the class again (`codegen/statics.rs::init_call`).

### 1.5 `instanceof`, patterns and casts

- `x instanceof Circle` is a test of the class's number, `matches!(x.0.tag(), 2)` (the numbers
  of every class at or below `Circle`); against a type that shares no variant
  with the operand, it is `false`, and when the operand's static type is already the tested type
  or below it, it passes (an operand with effects is still evaluated).
- A pattern variable is an owned value: the tested type's enum gets `as_circle(&self) ->
  Option<Arc<..>>` (`Need::Narrow`), and `x instanceof Circle c` binds `c` from it (a handle copy
  per successful test). One form covers a binding to a class and to a type with an enum of its
  own.
- Where a pattern variable is bound (`analysis/patterns.rs`): an operand of an `&&` chain (a let
  chain in an `if`, `else if`, `while` or `?:`, `is_some_and(|c| ..)` in any other expression);
  `if (!(x instanceof C c)) .. else ..` (`if let` with the branches swapped); and
  `if (!(x instanceof C c)) <leave>;` (`let .. else`, where leaving is `return`, `break`,
  `continue` or `throw`). An `if` whose condition is an `||` chain with a negated pattern is
  turned around into an `&&` chain of the negated operands with its branches swapped
  (`lower/negated_patterns.rs`). A read anywhere else is rejected: "a pattern variable used here
  not supported yet". A pattern variable may be assigned, except one of a record or switch
  pattern.
- A downcast `(Circle) s` is `java::cast(&s, |o| o.as_circle(), ("Circle", java::APP))`, which
  returns the value or throws `ClassCastException` with HotSpot's message: "class Square cannot
  be cast to class Circle (Square and Circle are in unnamed module of loader 'app')", or with each
  class's module of its own when they differ. The object's class comes from the enum's
  `java::CastSource`, a `match` written once per enum; the target's is written at the cast. A
  class's module is its package's named module, with the loader the JVM gives it (`bootstrap`,
  `platform`, or `app`, `program::loader_name`), or the unnamed module of the system class loader;
  the runtime's own classes (strings, boxes, arrays of primitives) are `java.base`'s, and an array
  is in its element's module.
  A cast whose operand's class is known and not below the target
  always fails, and is written as that failure. A cast or test to a supertype is the implicit
  upcast.
- A cast of a value that may be null to a Java enum is an `Option` of the enum, as any enum
  value that may be null is (`analysis/nullness.rs`).

Record patterns and patterns in `switch` are `if let` chains over the selector's enum built from the same tests.

### 1.6 Identity and null

`impl PartialEq` for an enum is identity: `Arc::ptr_eq` per variant, `==` for a Java enum's
constant (constants are singletons), `false` across variants. `==` on enum values is `&a ==
&b`, and with a value that may be null `a.as_ref() == Some(&b)`. Plain handles keep
`Arc::ptr_eq` and `java::same`.

A string or a box inside `Object` is a plain value whose identity the translation does not
keep, so `==` between two `Object`s where both sides may be strings or boxes is
`java::same_object` (`Facts::identity_checks`): different values are different objects, and two
equal ones end the program (`java::unkept_identity`), as Java may or may not see one object in
them. Never compare by value where Java compares references. Whether a side may be one is a points-to
analysis (`analysis/object_values.rs`): it follows, until nothing changes, whether each
variable, parameter, return value, field, static and array element type may hold a string or a
box, and which classes its objects may be of; each instance method is followed once per class
whose objects run it, with `this` of that class. An expression of one of the program's own
classes is never a string or a box, because its Rust type holds none. What the analysis does
not follow (a library call's result, the parameters of `equals` or `compareTo` that the runtime
may call) may be anything. Where comparing the values gives Java's answer, they are compared:

- the shortcut `a == b || a.equals(b)` (also with an assignment in an operand, as
  `HashMap.getNode` writes it), since the result is `equals`'s either way;
- in `TreeMap`'s own code, a comparison of two keys of its entries (or of a field that holds
  only one, `null` or a sentinel) compares by value. The keys of a sorted map are distinct under
  its comparator, and the translation's comparators see strings and boxes only by value, so two
  equal keys cannot both be in the map. The rule is that narrow because the argument rests on
  that invariant.

A box compared by identity as its own type (`Integer a == b`) is a shared `java::Boxed`
object for that kind throughout the program (`analysis/boxing.rs`, `Facts::faithful_boxes`).

`==` on two strings in the program's own code is decided by where each side's strings come from
(`analysis/string_identity.rs`). The analysis follows the strings backward through locals,
parameters, returns, fields, statics and array elements, through the program and the library
code that a whole program translates, to their sources: an interned string (a literal or
constant, `intern()`, an enum constant's name, a class's name, a canonical charset name), a new
one (`new String`, a concatenation that is not constant, a builder's or `String.format`'s text
that is not empty, a number's text), or one that Java keeps for a key (a system property,
`System.lineSeparator()`, an environment variable, a class's simple or canonical name). The
JDK's methods that return their receiver when nothing changes (`substring` of all of it, `trim`,
`strip`, the case mappings, `replace`, `concat("")`, `repeat(1)`, `split` without a match) and
`String.valueOf(o)` and `o.toString()` lead to what they may return. A string from a library
call that the analysis does not follow rejects the comparison, naming the call. The comparison
is then:

- by text where both sides can only be interned strings;
- `a == null && b == null` where no string can be on both sides: one side only interned and the
  other only new, or a side that is only ever null;
- otherwise by the objects at run time. Every string of the program is then a `java::JStr`, the
  address of an object in the collected heap that holds the text's length and its WTF-8 bytes
  (see 4), copied as one word and compared by address (`java::same_str`), and the program-wide switch
  (`Class::shared_strings`) makes each place that makes a string make the object Java makes: a
  literal is interned once (`java::interned_literal`), a new string is new, a method that may
  return its receiver returns it when Java's would (`java::kept` and its siblings, with the
  receiver held in a local), and what Java keeps per key is kept per key (`java::per_key`). A
  class's simple name is kept by its text, so two classes of the same simple name share one, and
  an environment variable by its value. A string held as an `Object` is compared the same way.

Setting `JAVA_TO_RUST_SHARED_STRINGS` for a translation shares every program's strings, which
checks the generator's shared strings against the corpus.

### 1.7 Object

`java.lang.Object` is a synthetic interface type of the program, `OBJECT` (`lower/object.rs`),
which every type without a superclass implements, so the machinery of interfaces (an enum over
the classes whose objects flow into it, conversions, dispatchers, `as_x`, `instanceof`) covers
it. Lowering adds it only to a program that mentions `java.lang.Object` (other than the
implicit `super()`), so other programs translate without it.

Runtime variants are types of kind `TypeKind::Runtime` beside it:

- strings and the eight boxes hold the plain value (`Object::String(String)`,
  `Object::Integer(i32)`); a primitive converted to `Object` becomes its box's variant;
- an array type converted to `Object` or cast back (`lower::object::note_object_array`) gets a
  runtime class named after it (`int[]`), whose variant holds the shared `java::Array`
  (`Object::IntArray(..)`), so the array keeps its identity: `==` and `equals` are `Arc::ptr_eq`,
  `hashCode` its identity hash, and it prints as Java's `[I@1b6d3586`. Arrays of a real generic's
  instantiation have no runtime class and are rejected;
- a `StringBuilder` held as an `Object` holds the shared `java::StringBuilder` (a
  `java::SharedStringBuilder` where two threads may reach builders): identity `==`, `equals`
  and `hashCode`, printed as its text (`ast::hierarchy::shared_of`);
- a class value held as an `Object` is a `Class` variant, equal and hashed as the class value;
- `new Object()` makes an object of `PlainObject`, a class without fields, equal only to itself
  and printed as `java.lang.Object@..`, as the JDK's lock objects and placeholders (`HashSet`'s
  `PRESENT`) need.

`equals(Object)` and `hashCode()` are two abstract methods of `Object`, which the program's
own `equals(Object)` and `hashCode()` override. A variant whose class overrides neither gets an
arm in the dispatcher: identity (`Arc::ptr_eq`, `java::address_hash`) for an object; value
equality and Java's hash for a string or a box (`Double` and `Float` compare `to_bits`, as
`Double.equals` does); `==` and the ordinal for an enum constant (Java's hash there is an identity
hash, which no program can depend on). The same call on a single class without an enum is the
identity directly (`identity_call`). `String.equals(Object)` and `Integer.equals(Object)` with
an `Object` argument are `Object`'s `equals` on the receiver converted to `Object`. An interface
that declares `equals` or `hashCode` again (`Comparator`) dispatches a class without its own to
`Object`'s. `Object.toString()` is the enum's `java_string()`, which calls each class's
`toString` and makes the text of the classes without one.

The identity hash is the address of the class's struct (`java::address_hash` of
`RefCell::as_ptr`, `Arc::as_ptr` or `ptr::from_ref`), so a method that borrows the object hashes the same as code holding its handle. `System.identityHashCode` uses it too. A class
without `toString` prints as `java::default_text(name, hash)`: its binary name
(package-qualified, `$` between nested classes), `@` and the hash in hexadecimal, using the
class's own `hashCode` when it has one. The hash differs from Java's, as Java's differs from run
to run.

### 1.8 Shared, immutable, owned under inheritance

- **Owned** (a plain value, `analysis/owned.rs`): never a class with subclasses, a class that is
  ever converted to a supertype, a class whose objects run an inherited method that is not
  field-level or a `super.m()` of one (`runs_inherited_object_code`), or a class used as a type
  argument (a generic holds its values as `T: Clone`).
- **Immutable** (`Arc<C>`, `analysis/objects.rs::immutable_classes`): decided on the whole object
  (a write of an inherited field by any method makes the family mutable), then unified per
  family, so `base()` has one type for the whole enum: `Ref<'_, XFields>` for a family behind
  `RefCell`s, `&XFields` for an immutable or celled one. The cost is that a leaf loses `Arc<C>`
  when a sibling is mutable. Interface enums and `Object` may mix `Arc<C>` and `Arc<RefCell<C>>`
  variants.
- **Celled** (`Arc<C>` whose fields are `java::Field` cells, `analysis/objects.rs::celled_classes`):
  a mutable family none of whose classes is owned, reachable by a second thread, or locked
  (a kept monitor). Every field of the family is a cell: a read copies the value out
  (`o.count.get()`), a write puts a new one in (`o.count.set(v)`), so no borrow is held and
  none can conflict, and there is no borrow flag. Methods take `&self`, and `base()` lends the
  fields struct as `&XFields`. A field assignment whose object may be null computes the value
  first, as Java does. The families that threads share keep their `java::Locked`.
- **Fixed fields** (`analysis/fixed_fields.rs`): a field of a mutable class that only
  constructors write, through `this` (a `final` field, or one written the same way), keeps its
  value once the object is built. Its read on a concrete receiver is `java::fixed!(c, base.name)`
  rather than `c.borrow().base.name`: the field itself, borrowed as long as the handle, with no
  `RefCell` borrow or lock taken and none to end, so it needs no copy where a borrow of the
  object could not last (`{ c.borrow().name.clone() }`). In a program with threads, the fields of
  a class whose constructors, or whose subclasses' constructors, may start a thread are not
  fixed, since that thread could read one while the constructor writes it. The runtime's
  `fixed.rs` gives the reasons that the borrow cannot alias a write.
- A fields struct has no representation of its own: it is always inside a concrete object.

### 1.9 Comparable, Number, CharSequence, Enum and other JDK interfaces

A class may implement `Comparable<T>` and call `compareTo` directly with no extra machinery.
When the program uses one of these JDK types as the type of a value, lowering adds it as a
synthetic interface of the same kind as `Object` (`lower/object.rs`), which runtime types and
the program's classes implement:

- `COMPARABLE` (a bound such as `T extends Comparable<T>`, a `Comparable[]`): strings, boxes, the
  program's enums and its classes that implement `Comparable`. Its abstract
  `compareTo(Comparable)` has an arm per runtime type and enum (`codegen::dispatch::compare_arm`),
  which throws `ClassCastException` for an argument of another class. A class's own
  `compareTo(C)` implements it through a bridge (1.11).
- `NUMBER` (`T extends Number`, `List<? extends Number>`): the six numeric boxes, whose
  `byteValue()` to `doubleValue()` convert with `as` (`number_arm`), and the program's classes
  that extend `Number` (the JDK's `AtomicInteger`, `AtomicLong`, `Striped64`, `BigInteger`,
  `BigDecimal`). For those, `Number` counts as `Object` (`super()` does nothing), and the
  `intValue()` to `doubleValue()` they declare or inherit implement the interface's. `Number`'s
  own `byteValue()` and `shortValue()` are concrete, `(byte) intValue()` and `(short)
  intValue()`, and the boxes agree, so every call of them lowers to that narrowing; a class that
  overrides them is rejected.
- `CHAR_SEQUENCE` (the JDK's `StringJoiner` and `Collectors.joining` take `CharSequence`s):
  strings, builders and the program's classes that implement it. `length()`, `charAt`,
  `subSequence` and `getChars` answer as the string, the builder or the class does
  (`char_sequence_arm`); a class without `getChars` gets the interface's default, written as the
  JDK has it. `toString()` is the value's text, `isEmpty()` a zero length. The JDK's
  `CharSequence` is not translated (it is among `RUNTIME_CLASSES`), and its other methods are
  rejected.
- `ENUM` (`<E extends Enum<E>>`, the JDK's `EnumMap` and `EnumSet`): the program's enums, with
  `Enum`'s methods lowered by `lower/enums.rs`.
- The JDK's `java.lang.Appendable` is translated, and the runtime's builders implement it.

Other rules:

- An interface method that a Java enum implements with a method every enum has (`name()`,
  `ordinal()`, `toString()`) dispatches to the enum's own; any other method of `java.lang.Enum`
  doing so is rejected.
- A class may implement an interface's method with a method it inherits from a superclass that
  does not implement the interface (`Spliterators.EmptySpliterator.OfInt`). javac's `overrides`
  cannot list it, so `JavaTree` gives the class an `inherits` list of such pairs, and
  reachability and lowering add the interface's method to the overrides of the inherited one
  (with a bridge when erasure changes its signature).

### 1.10 Printing

A class's `toString` is a method returning the string (a `java::JString`), never a `Display`:
`Display` writes text, which would show a lone surrogate as `?` where the string is used as a
value, and would not keep the object that a shared string is. Printing and concatenation call
it. An enum's `java_string()` dispatches per variant to the class's `toString`, or to the
inherited body, and its `fmt::Display` (for `printf` and the other text the runtime makes) shows
that string; in a concatenation the enum value is a `java::Piece` that pushes its
`java_string()`. An inherited `toString` emitted on an enum is always `to_string_in_<type>`, since
clippy forbids an inherent `to_string` beside `Display`. `java::or_null` works for
`Option<Enum>`.

### 1.11 Generics

**Erased** (`lower/generics.rs`). Lowering erases generics, and nothing after it knows they
existed:

- A type variable is the erasure of its first bound (`Object` without one, `Comparable` for
  `T extends Comparable<T>`), looked up in the method and then its class and the classes it is
  nested in. A type variable hides a class of the same name.
- A parameterized type is its class; `? extends X` and `capture#1 of ? extends X` are `X`; `?`
  and `? super X` are `Object`. A capture of those has its type variable's bound, so
  `JavaTree` writes it as a capture of `? extends` the bound's erasure (`ChronoLocalDateTime<?>`'s
  `toLocalDate()` is a `ChronoLocalDate`). javac writes an inner class of a generic class with the outer's
  type arguments (`EnumMap<K,V>.EntryIterator`); those are dropped first
  (`without_outer_args`).
- Members are declared with erased types. A use whose type javac substituted (a call's result, a
  field, an element) gets `Cast(substituted, use)`, which is javac's checkcast and becomes
  `java::cast(x.as_integer(), ..)`. Arguments convert through the conversions every call makes.
- A type variable with several bounds (`T extends Object & Comparable<? super T>`, as
  `Collections.max` has) erases to the first, and a call of a later bound's method casts the
  receiver to that bound (`generics::receiver`). An intersection type written elsewhere is
  rejected ("intersection types").
- A varargs parameter `T...` is an array of the erasure.
- Generic constructors, explicit type arguments, raw types and diamonds need nothing of their
  own.

**Bridges** are methods (`lower/object.rs::bridges`). When a method overrides one whose erased
parameter or return types differ (`Integer next()` for `T next()`, `compareTo(Point)` for
`Comparable`'s), lowering adds a method with the overridden signature that converts the
arguments, calls it and converts the result, and that method takes its place in the override
group. The method keeps its own types and a name told apart by its types (`next_integer`,
`compare_to_point`). A subclass whose superclass already has the bridge gets none.

**Covariant returns** (`codegen/covariant.rs`): a dispatcher returns the overridden method's
representation, so an override's value is wrapped in its variant or converted with
`to_<supertype>()`.

**Covariant arrays** (`lower/covariant.rs`). Java's arrays of references convert to arrays of a
supertype (a `Version[]` is an `Object[]`), and erasure makes `T[]` an array of the bound, so
`list.toArray(new Version[0])` passes a `Version[]` where `T[]` is an `Object[]`, and `toArray(a)`
fills `a` and returns it. Copying at the boundary would lose that identity. Instead, two different
array types of references that meet (an argument, an assignment, a return, a cast, a substitution
for an array of a type variable, an array passed for `Object...`) are each an `Object[]`
everywhere in the program: lowering notes each such type and the program is lowered again
(`real_generics::retry`). Element reads get the checked cast back, a for-each casts each element,
and a store converts to `Object`. Every other array keeps its exact type.

Such an array carries the class it was made of (`java::ElementClass`, behind the array's handle
next to its elements; every other array carries `None`, one word). Lowering notes the class of
each array the code makes where it is an `Object[]` (`Class::element_classes`): a `new String[n]`
or `{a, b}`, and an `Arrays.copyOf` or `copyOfRange` into a class literal, or into a class held in
a variable, which is taken as `Object[]`. The `String[]`s that the runtime makes as `Object[]`s
(`split`, `main`'s arguments) carry `String[]`. A clone or a copy without a class keeps its
source's class. So:

- a store into an `Object[]` (`java::checked_set`, `Class::checks_store`) checks the index, then
  whether the array can hold the object, by the names of the object's class and of every class
  and interface above it: an object of another class throws `ArrayStoreException` with its class
  name, as HotSpot's does. A store into an array that carries no class costs one test of the
  handle more than another store;
- `System.arraycopy` into an array that carries another class than the source's checks each
  element, and stores those before the first that fails, which throws HotSpot's
  `ArrayStoreException` ("arraycopy: element type mismatch: ...");
- `getClass()` of an `Object[]` (`java::array_class`), as a value or as an `Object`, is the class
  it carries.

An array of arrays (`String[][]` as an `Object[][]`) carries no class, nor does an array that a
method keeps to itself as a plain `Vec` (`analysis::arrays`), an array that
`Array.newInstance(a.getClass().getComponentType(), n)` makes, or an enum's `values()` as an
`Object[]`: storing an object of another class in one of them does not throw, and its `getClass()`
is `Object[]`'s. `instanceof String[]` and a cast to `String[]` of an `Object` do not look at the
class an array carries. Also:

- a `String[]` that is an `Object[]` is copied where the runtime's string builtins take or make
  one (`Builtin::StringArrayOf`, `Builtin::ObjectArrayOf`; the copy of a null is null, so
  `Arrays.equals` of two null fields is `true`), which only matters where the builtin
  keeps the array. `Arrays.sort` and `Arrays.fill` of one, which change it in place, run the
  JDK's code for an `Object[]` with the JDK's sources (`Reach::string_array_writes`) and are
  rejected without them ("a change in place of a `String[]` that is also an `Object[]`");
- an array of an enum that is an `Object[]` holds its constants as `Object`s, cast back where
  read (an `Option` where the element may be null).

`System.arraycopy` where the source or destination is an `Object` matches their array variants
(`codegen::dispatch::object_array_copy`, `codegen::arraycopy`): two arrays of one type are copied,
and a `String[]` and an `Object[]` are copied element by element, where an element that is not a
`String` throws `ArrayStoreException` after the elements before it are stored. Two arrays of other
classes are copied as two typed arrays are, below; an element whose class no value of the
destination's element class is anywhere else has no variant to be stored as, and ends the program
(the analysis does not follow which arrays an `Object`-typed copy is given: the JDK's
`SpinedBuffer` copies only arrays of one type). Other mismatches throw `ArrayStoreException` with
HotSpot's message for them ("arraycopy: type mismatch: can not copy int[] into long[]",
"arraycopy: source type java.lang.String is not an array"). A pair of arrays of
two other element classes is copied as HotSpot copies it (`codegen::arraycopy_classes`): when the
source's class is a subtype of the destination's, every element is stored; otherwise each element is
checked as it is stored, where null always passes and a non-null element must be an instance of the
destination's class. The first element that fails throws `ArrayStoreException` after the elements
before it are stored (`runtime::try_copy_refs`). The message names the two element classes by their
static types, since a typed array holds no runtime class of its own; for arrays held as `Object`s,
by the classes of their variants.

A value read back out of an erased container is a cast from `Object` that may be null, so an
enum read back is an `Option` of the enum.

**Real Rust generics** (`lower/real_generics.rs`). Realness is decided in lowering, since the
erased program has lost what the decision needs. Candidates:

- a class whose type parameters have no bounds and each occur in the type of an instance field
  (a Rust struct must use its type parameters), that extends and implements nothing, has no
  subclasses and is not abstract;
- a method whose type parameters have no bounds and each occur in a parameter's type (so Rust
  can infer them).

Lowering translates them as real, then `real_generics::verify` fails every candidate whose type
variables' values are used for anything but a store, a return, an argument to a parameter of a
type variable or a comparison with `null`; whose types meet a cast or an `instanceof`; that is
used raw, as an array element, or becomes an `Object`. Lowering runs again with the failed ones
erased until nothing fails, and an error with candidates left is retried with all of them erased,
so a real generic never turns a translated program into a rejected one.

A type variable of a real generic is a synthetic type of kind `TypeKind::Param`, named
`Owner#T`, with no members and no implementors, so every analysis sees an opaque value that is
stored, moved and cloned. `Type::Class` carries its type arguments (`Type::Class("Box",
[String])`). Type parameters are bounded by `Clone` only (`impl<T: Clone> Box<T>`), and every use
of a value clones it. A literal passed for a type variable gets its type suffix (`3i32`). Use
sites keep javac's substituted types, with no cast.

### 1.12 Interfaces: default and static methods, constants, enums implementing interfaces

- An interface is a type like any other: an enum over the classes whose objects flow into it,
  or that class's object when there is one implementing class.
- An interface whose only implementor is a Java enum is still an enum with one variant
  (`Coded::Color(Color)`), so that a value of a class type is always an object (`Option` for
  null, identity through the enum).
- A Java enum implementing an interface is a variant holding the `Copy` constant: compared with
  `==`, shown through the enum's `Display`, converted with `*o`, its methods called as `o.m()`
  (they take `self`).
- Default methods are object-level: on the interface's enum, in the one implementing class's
  `impl`, or for an enum in the enum's `impl` with `self` by value.
- Static methods are free functions; constants are `const` items or cells like a class's
  statics.
- javac does not mark an interface's fields `static` or its methods `abstract` in the tree, so
  `STATIC` comes from the symbol and a method without a body is abstract.
- A parameter that a body never mentions is named `_x`, so a default meant to be overridden has
  no unused-variable warning.

### 1.13 Freeing objects

Objects live in the collected heap (docs/collector.md). A handle is a pointer with no count, and
an object is never dropped: the collector frees whatever no handle, field or array element
reaches, cycles included, as Java's does. Freeing never recurses through fields, so a linked
list of a million nodes needs nothing special (`corpus/features/LongChainsDrop.java`).

### 1.14 `Object.clone`

`super.clone()` of `Object` is a copy of the object's runtime class, as Java's is
(`codegen/object_clone.rs`). On the enum of a class tree it is a `match` with a struct literal
per variant, which reads each field of the variant's struct and of the fields structs it embeds,
cloning a field the way its type's values are cloned (handles are shared, as Java shares the
references). `(C) super.clone()` in `C` is the copy itself. Whether a class may be copied is
`TypeDecl::cloneable` (it, a superclass or an interface extends `Cloneable`, read from javac's
tree). A variant that is not `Cloneable` throws `CloneNotSupportedException` with its binary
name (`java::clone_refused`), which a `catch` sees (`java::try_clone_refused`), so the `try`s
of `EnumSet.clone` and `ArrayList.clone` translate. `clone` of another object than `this` is
rejected.

## 2. Rules the analyses follow

A call may run any member of an override group, so the analyses treat the group as one:

- **Parameter and return forms** (`borrowed_params`, `borrowed_objects`, nullable parameters and
  returns, ...) are unified over each override group (`analysis::align_groups`): a form stays only
  if every member has it, and nullness spreads to all (`analysis/nullness.rs`). Dispatchers then
  forward arguments unchanged.
- **`mutating` and `writers`** are spread over override groups, and `isolated` holds only for
  whole groups (`analysis/borrowing.rs`), which keeps the `&mut self` choice and the
  borrow-conflict counts right for a call that may run any member.
- **Field keys**: nullable fields are keyed by the declaring class (`Facts::field_owners` maps
  (class, field) to it), since a receiver may be a subclass or an enum. A field declared in `D`
  is set by construction when `D`'s constructors set it, as the subclass's literal embeds them.
- **Borrowing through enums**: a `Ref` from `base()` must not live across a mutable borrow of the
  same object. Field reads of enum-typed objects are scoped like those of any shared object, and
  a dispatcher call counts as a borrow that may write.
- **Analysis order**: `analysis::analyze` returns a `Result`, since rejecting a superclass
  constructor that is not a plain-value builder needs the borrowing analysis
  (`analysis::validate` holds the rejections that depend on facts).
- A variable mentioned twice in one statement is not moved there, since Rust keeps the
  statement's temporaries (a `Ref` of a field read, `format_args!`'s references) alive to its
  end; a mention in the condition of `?:` does not count. A `for` loop's update is scoped like a
  statement, and a field of `this` assigned from a borrow of itself (`head = head.next`) is a
  block of its own (`codegen::objects::reassigns_through_borrow`).

## 3. Not done

- Real generics need no trait: a generic whose type variables are used for text, `equals`,
  `hashCode`, a bound, a cast or an array stays erased. Traits for interfaces used as bounds and
  per-instantiation enum variants (`Shape::BoxOfInteger`) are not built.
- Arrays of arrays, plain arrays and the arrays that `Array.newInstance` and `values()` make as
  `Object[]`s do not carry their class (1.11, covariant arrays), and `instanceof` and casts
  of an array as an `Object` do not look at the class an array carries.
- `Number.byteValue()` or `shortValue()` overridden by a program class, `CharSequence`'s methods
  beyond those listed in 1.9, and other `java.lang.Enum` methods implementing an interface method
  are rejected.
- A class with subclasses that needs the whole-object constructor form, whose enclosing
  instance or captured locals a `super(..)` computes, is rejected (1.4).

## 4. Strings as WTF-8

A translated string holds exactly Java's UTF-16 code units, lone surrogates included, while
normal text stays plain UTF-8 at no cost:

- **The bytes.** A string is WTF-8: UTF-8 that also encodes a lone surrogate in UTF-8's 3-byte
  form (`ED A0..BF ..`). A surrogate pair is always its 4-byte character, never two 3-byte
  halves, so each Java string has exactly one encoding, and equality, ordering and hashing are
  byte comparisons. Text without lone surrogates is byte for byte its UTF-8.
- **The types** (`runtime/src/wtf8.rs`). `java::Wtf8` is the unsized string slice, a `[u8]` that
  holds WTF-8, with the `str`-like methods the generator and the runtime use; `java::JString` is
  the plain owned string (a `Vec<u8>`), and `java::JStr` the shared one (the address of its length
  and bytes in the collected heap, `Copy`, see 1.6); both deref to `Wtf8`. `Type::String` is `java::JString` (or `java::JStr`
  where strings are shared), a borrowed string parameter is a `&java::Wtf8`, and a literal is
  `java::wtf8("..")`, a `&'static Wtf8` view of its UTF-8 made at compile time. A literal with
  a lone surrogate, which javac's tree gives as its code units, is
  `const { java::wtf8_bytes(b"..") }`: its WTF-8 bytes, checked when Rust compiles the program;
  a `case` label folded from such literals matches the same bytes. The runtime's
  string functions take `&Wtf8` and return `JString`; a function that needs a `&str` (a
  pattern, a format string, a path) checks for lone surrogates first: a path or a number to
  parse turns one into `?` as Java does, a pattern ends the program loudly. A regular
  expression matches the code points of its input, a lone surrogate being one of its own, as
  Java's `Pattern` does.
- **Joining.** Appending text that starts with a low surrogate to text that ends with a high
  one joins the two into their 4-byte character, as Java's UTF-16 shows them as one pair: a
  check of the bytes at the seam, in `JString`'s pushes. A concatenation is `java::concat!` (and
  `s += ...` is `java::append!`), whose pieces (`java::Piece`) are pushed one after another:
  each string, `char`, builder or object's string as it is, and the text between them as a
  literal or a `format_args!` of numbers and the like, so that halves that meet join.
- **Code units.** `charAt`, `substring` and the other unit-indexed methods keep their ASCII fast
  paths (`charAt` and `hashCode` read the ASCII bytes before the first other character as they
  are, and decode only from there); their other path decodes WTF-8 to units exactly and encodes a lone surrogate back
  (`from_units`). A builder holds UTF-16 units, so its `toString` (`text()`) is exact.
- **Objects.** A `toString` is a method returning the string, never a `Display` (see 1.10), so
  the string it returns is kept exactly; an enum's string is `java_string()`, and in a
  concatenation an enum value is a piece that pushes it.
- **Output.** Printing encodes as Java's UTF-8 `PrintStream` does: `Display` of a string shows a
  lone surrogate as `?`. A printed line with two or more strings or `char`s, which could be the
  halves of a pair, is made first and then printed.
- **`switch` and `valueOf`.** A string `switch` and an enum's `valueOf` match the string's bytes
  against byte string literals (`b"..."`), a string constant in a `case` by its text.

What does not hold a lone surrogate exactly: the message of an exception the runtime raises
(a Rust `String`), a thread's name (Rust's), and the text of a charset or a `Class`.

The translated javac keeps a lone surrogate exactly too: a `char` literal such as `'\uD800'` is
compiled into the same class file as the real javac compiles it (`features-CodePoints` and the
other corpus cases of `scripts/javac-check.sh --corpus`). WTF-8 costs javac no measurable time:
a debug build of it compiles `corpus/programs` in the same time as with Rust strings, within the
noise of a loaded machine, and builds with a rustc peak of 8.0 GB against 7.9 GB.
