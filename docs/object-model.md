# The object model: inheritance, the Object root, interfaces, generics, instanceof and casts

This plan was written for an agent that implements it from this document alone, and is kept
up to date as decisions change (see "Deviations" at the end). It builds on the repository as of
commit `1d1a86a` (`crates/translator/src/{ast.rs, lower.rs, lower/, analysis.rs, analysis/,
codegen.rs, codegen/, program/}` and `crates/runtime`). Where a choice was open, the choice and
its reason are given. The representative Rust shapes of section 1 were checked to compile and
run against the runtime crate before the implementation started.

Decisions already made by the user and kept here: the enum way for hierarchies (closed
world), real Rust generics where they work and an erased form elsewhere, exceptions are
`Result`-based later (not in this plan), `String` stays hand-written in the runtime.

## 0. The decisions in one page

1. **A concrete class is a struct, as today.** Its inherited fields are *embedded*: the fields
   declared by its superclass chain sit in a field `base` whose type is the superclass's
   *fields struct* (`struct Circle { base: BaseFields, r: f64 }`). A class that has subclasses
   in the program gets a fields struct named `<Name>Fields` holding only the fields it declares
   itself (plus its own `base`), and only when it declares instance fields. Embedding (not
   flattening) because `super(...)` becomes the `base:` entry of the constructor's struct
   literal, inherited methods that only touch fields are emitted once on the fields struct, and
   an enum needs one `base()` accessor instead of one accessor per inherited field.
2. **A type with several concrete classes is an enum named after the Java type**, with one
   variant per concrete class that can actually be held by that type (see "variants"), each
   wrapping that class's existing representation (`Rc<RefCell<C>>`, or `Rc<C>` when immutable).
   A type with exactly one concrete class is that class's representation (no enum), as the
   user decided. Enums are flat (every variant is a concrete class, never another enum), so an
   interface over classes from several class trees is one `match` deep.
3. **Dispatch is a `match` in a method of the enum** (`fn area(&self) -> f64 { match self {
   Base::Circle(o) => o.borrow().area(), Base::Rect(o) => o.borrow().area() } }`). The body a
   class inherits lives once: on the superclass's fields struct when it uses `this` only for
   fields (a *field-level* method), otherwise on the enum of the declaring type (an
   *object-level* method), named `m` when nothing overrides it and `m_in_<declaring class>`
   when a dispatcher `m` also exists.
4. **Conversions are explicit in the AST**: lowering inserts `Cast(target, expr)` at every
   implicit reference conversion (assignment, argument, return, array element, conditional
   arm, field initializer), so analyses see every upcast and the generator writes
   `Base::Circle(c)` (concrete to enum), `b.to_shape()` (enum to enum, generated), or
   `s.to_circle()` (downcast, generated, throws `ClassCastException`).
5. **`instanceof` is `matches!`/`if let` on the enum**, a pattern `x instanceof C c` binds `c`
   with `if let Base::Circle(c) = &b`, and `if (!(o instanceof C)) return ..; C c = (C) o;`
   becomes `let Object::C(c) = &o else { return .. };`.
6. **`Object` is an enum** over every type that flows into an Object-typed place: program
   classes, `String(String)`, the boxes (`Integer(i32)`, faithful `Boxed<i32>` where identity
   is observed), Java enums, and later generic instantiations and arrays. Its `equals`,
   `hash_code` and `Display` dispatch to the class's own method or to the defaults (identity,
   identity hash, `Name@hex`). Printing an object without `toString` is no longer rejected; it
   prints `Name@<hash>` like Java (the hash differs from Java's, as Java's differs run to run).
7. **Identity is `PartialEq` on the enums** (pointer equality per variant, `false` across
   variants), so `a == b` and `Option` comparisons read as in Java; `Rc::ptr_eq`/`java::same`
   stay for plain handles.
8. **Representation choice (shared / immutable / owned) stays per concrete class**, with three
   inheritance rules: a class that is ever converted to a supertype, has subclasses, or calls
   an inherited object-level method is never owned; the concrete classes of one class tree (one
   *family*) are immutable together or not at all (so one `base()` accessor type works for the
   whole enum); interface enums and `Object` may mix representations.
9. **Generics: erased first, real where proven.** Every generic class and method is first
   translated erased exactly as javac does (type variables become their bound, `Object` by
   default; use sites get checkcasts as `Cast` nodes). A generic class or method whose type
   parameters are used only in typed positions, with fully parameterized, wildcard-free use
   sites, is then upgraded to a real Rust generic (`struct Box<T>`, `fn max<T: Comparable<T>>`),
   with interfaces used as bounds generated as traits implemented on the classes'
   representation types. This is the project's own order: the faithful form first, analyses
   refine it.
10. **Exceptions** fit without change: exception classes are a class tree under a runtime base
    (`java::Throwable` fields struct, added by the exceptions agent), `catch (T e)` is a match
    on the variants of `Throwable` that are concretes of `T`, with the conversions of section
    1.6.

## 1. The representation

### 1.1 Vocabulary and the hierarchy table

The AST gets a hierarchy table, built by lowering from javac's `extendsClause`,
`implementsClause` and each method symbol's `overrides` list:

```rust
// ast.rs
pub enum TypeKind { Class, Interface, Enum }

pub struct TypeDecl {
    pub name: String,
    pub package: String,
    pub kind: TypeKind,
    pub is_abstract: bool,
    /// The superclass, an index into `Class::types`; `None` for Object, interfaces and enums.
    pub extends: Option<usize>,
    /// Interfaces implemented (classes) or extended (interfaces), indexes into `Class::types`.
    pub implements: Vec<usize>,
    /// Runtime supertypes the program does not declare: today only `java.lang.Comparable`
    /// in single-file mode (see 1.9); the exceptions agent adds `java.lang.Throwable`.
    pub runtime_supertypes: Vec<&'static str>,
    pub type_params: Vec<TypeParam>,          // milestone 5
    pub fields: Vec<Field>,
    pub statics: Vec<Field>,
    pub enum_constants: Option<Vec<EnumConstant>>,
}

pub struct TypeParam { pub name: String, pub bounds: Vec<Type> }

pub struct Method {
    // as today, plus:
    pub is_abstract: bool,            // no body: abstract, or an interface method without one
    pub type_params: Vec<TypeParam>,  // milestone 5
    /// Methods this one overrides (indexes), from javac's `overrides` list.
    pub overrides: Vec<usize>,
}

pub struct Class {
    pub name: String,
    pub types: Vec<TypeDecl>,
    pub methods: Vec<Method>,
    pub hierarchy: Hierarchy,   // computed by `Hierarchy::new(&types, &methods)` at the end of lowering
}

pub struct Hierarchy {
    /// Direct subtypes of each type (classes extending it, interfaces extending it, classes implementing it).
    pub subtypes: Vec<Vec<usize>>,
    /// Methods that directly override each method.
    pub overriders: Vec<Vec<usize>>,
}
impl Hierarchy {
    /// Whether type `a` is `b` or a subtype of it (transitively, through classes and interfaces).
    pub fn is_subtype(&self, a: usize, b: usize) -> bool;
    /// The superclass chain of `t`, nearest first (not including `t`).
    pub fn ancestors(&self, t: usize) -> Vec<usize>;
    /// The concrete classes (kind Class, not abstract) that are `t` or subtypes of `t`, in `types` order.
    pub fn concretes(&self, t: usize) -> Vec<usize>;
    /// Every method in the override group of `m`: `m`, what it overrides, what overrides those,
    /// transitively. Signatures (parameter forms, nullness) are unified over a group.
    pub fn group(&self, m: usize) -> Vec<usize>;
    /// The implementations a call resolved to `m` may run: the non-abstract members of the group
    /// declared in `t` or a subtype of `t`, where `t` is the receiver's static type.
    pub fn targets(&self, m: usize, t: usize) -> Vec<usize>;
    /// The implementation that an object of concrete class `c` runs for method `m`: the member
    /// of `m`'s group declared in the nearest class of `c`'s superclass chain (or `c`'s
    /// interfaces, for a default method), `None` when abstract (cannot happen for a concrete class).
    pub fn implementation(&self, m: usize, c: usize) -> Option<usize>;
    /// The class declaring instance field `name` visible from type `t`: `t` or an ancestor.
    pub fn field_owner(&self, types: &[TypeDecl], t: usize, name: &str) -> usize;
    /// The path from an object of class `c` to the fields struct of its ancestor `d`: `"base.base"`.
    pub fn base_path(&self, c: usize, d: usize) -> String;
    /// The nearest class at or above `t` that declares instance fields (the type of `base()`).
    pub fn fields_root(&self, types: &[TypeDecl], t: usize) -> Option<usize>;
    /// The root of the class tree of `t` (its topmost program superclass) and the family of
    /// `t`: the concrete classes under that root.
    pub fn family(&self, t: usize) -> Vec<usize>;
}
```

Terms used below:

- **leaf class**: a concrete class with no subclasses in the program. Its struct is named
  as today (`Circle`).
- **fields struct**: for every class that has subclasses in the program *and* declares
  instance fields: `struct <Name>Fields { base: <Super>Fields, <own fields> }` (the `base`
  entry only when an ancestor has instance fields). Abstract or not makes no difference. A
  class with subclasses that is itself instantiated has both a fields struct and, as the
  variant for objects that are exactly that class, `Enum::<Name>(Rc<RefCell<<Name>Fields>>)`.
- **single-concrete type**: a type (class, abstract class or interface) with exactly one
  concrete class under it. It has no enum; places of that type hold the concrete class's
  representation; its object-level methods are emitted in the concrete class's `impl`.
- **enum type**: a type with two or more variants. `Object` is an enum type whenever
  anything flows into it.
- **variants(T)**: the concrete program classes `c` under `T` that are not owned, in
  `Class::types` order, followed by the runtime types converted to `T` somewhere in the
  program (fixpoint through enum-to-enum conversions): `String`, the boxed kinds, Java enums,
  later generic instantiations and arrays. A concrete class under `T` that no code ever
  converts to a supertype is owned-eligible and absent from every enum; `instanceof` and
  casts naming it are then statically false/failing.
- **family**: the concrete classes of one class tree. Decides immutability together (1.8).
- **field-level method**: an instance method declared in class `D` (with a fields struct)
  that uses `this` only to read or write instance fields declared in `D` or its ancestors and
  to call field-level methods declared in `D` or its ancestors that are not overridden anywhere
  in the program; and never uses `this` as a value, never calls a method with overriders, never
  converts `this`, never calls `super.m()` of an object-level method. Emitted as
  `impl DFields { fn m(&self | &mut self, ..) }`.
- **object-level method**: every other instance method of a type that is not a leaf class:
  interface default methods, methods that call a method with overriders (virtual call) or an
  abstract one, that pass/store/return/compare `this`, or that convert `this`. Emitted on the
  enum (`impl D { fn m(&self, ..) }`, `self: &D`) or, for a single-concrete `D`, in the
  concrete class's `impl` as if declared there.
- Leaf classes keep today's rules for their own methods (`&self`, `&mut self`, or
  `this: &Rc<RefCell<C>>`) with the additions of section 2.

Naming: structs and enums use `struct_name` (`Outer.Inner` is `OuterInner`); the fields
struct appends `Fields`; variants are the concrete class's struct name, runtime variants
`String`, `Integer`, `Long`, `Double`, `Float`, `Short`, `Byte`, `Character`, `Boolean`, a Java
enum's struct name, `<Elem>Array` (milestone 5) and `<Class>Of<Args>` (milestone 6).
Accessors: `base()`, `base_mut()`. Conversions: `to_<snake>()` (throwing for downcasts),
`as_<snake>()` returning `Option` (for pattern binds to enum types). Inherited bodies:
`<m>_in_<declaring class snake>`. A program class named like a runtime variant (`String`,
`Integer`, ...) is rejected: "a class named like a runtime type".

### 1.2 A class hierarchy with an abstract base and overrides

```java
interface Shape { double area(); default String describe() { return "shape " + area(); } static int sides() { return 0; } }
abstract class Base implements Shape {
    protected String name; int uses;
    Base(String name) { this.name = name; }
    String getName() { uses++; return name; }                // field-level
    public String toString() { return name + ":" + area(); } // object-level: calls the abstract area()
}
class Circle extends Base {
    final double r;
    Circle(double r) { super("circle"); this.r = r; }
    public double area() { return 3.0 * r * r; }
    public String toString() { return "C" + super.toString(); }
}
class Rect extends Base {
    double w, h;
    Rect(double w, double h) { super("rect"); this.w = w; this.h = h; }
    public double area() { return w * h; }
    public String describe() { return "rect " + w + "x" + h; }
}
```

```rust
struct BaseFields {
    name: String,
    uses: i32,
}

impl BaseFields {
    fn new(name: &str) -> BaseFields {
        BaseFields { name: name.to_string(), uses: 0 }
    }

    fn get_name(&mut self) -> String {
        self.uses = self.uses.wrapping_add(1);
        self.name.clone()
    }
}

struct Circle {
    base: BaseFields,
    r: f64,
}

impl Circle {
    fn new(r: f64) -> Circle {
        Circle { base: BaseFields::new("circle"), r }
    }

    fn area(&self) -> f64 {
        3.0 * self.r * self.r
    }

    // `super.toString()` is an object-level body of Base, so `this` is needed as a handle.
    fn to_string(this: &Rc<RefCell<Circle>>) -> String {
        format!("C{}", Base::Circle(this.clone()).to_string_in_base())
    }
}

struct Rect {
    base: BaseFields,
    w: f64,
    h: f64,
}

impl Rect {
    fn new(w: f64, h: f64) -> Rect {
        Rect { base: BaseFields::new("rect"), w, h }
    }

    fn area(&self) -> f64 {
        self.w * self.h
    }

    fn describe(&self) -> String {
        format!("rect {}x{}", java::Double(self.w), java::Double(self.h))
    }
}

#[derive(Clone)]
enum Base {
    Circle(Rc<RefCell<Circle>>),
    Rect(Rc<RefCell<Rect>>),
}

impl Base {
    fn base(&self) -> Ref<'_, BaseFields> {
        match self {
            Base::Circle(o) => Ref::map(o.borrow(), |o| &o.base),
            Base::Rect(o) => Ref::map(o.borrow(), |o| &o.base),
        }
    }

    fn base_mut(&self) -> RefMut<'_, BaseFields> {
        match self {
            Base::Circle(o) => RefMut::map(o.borrow_mut(), |o| &mut o.base),
            Base::Rect(o) => RefMut::map(o.borrow_mut(), |o| &mut o.base),
        }
    }

    fn area(&self) -> f64 {
        match self {
            Base::Circle(o) => o.borrow().area(),
            Base::Rect(o) => o.borrow().area(),
        }
    }

    // a forwarder: one target, field-level
    fn get_name(&self) -> String {
        self.base_mut().get_name()
    }

    // the body declared in Base; `to_string` itself is `Display` (1.10)
    fn to_string_in_base(&self) -> String {
        format!("{}:{}", self.base().name, java::Double(self.area()))
    }

    fn describe(&self) -> String {
        match self {
            Base::Circle(_) => self.to_shape().describe_in_shape(),
            Base::Rect(o) => o.borrow().describe(),
        }
    }

    fn to_shape(&self) -> Shape {
        match self {
            Base::Circle(o) => Shape::Circle(o.clone()),
            Base::Rect(o) => Shape::Rect(o.clone()),
        }
    }
}

impl PartialEq for Base {
    fn eq(&self, other: &Base) -> bool {
        match (self, other) {
            (Base::Circle(a), Base::Circle(b)) => Rc::ptr_eq(a, b),
            (Base::Rect(a), Base::Rect(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Base::Circle(o) => f.write_str(&Circle::to_string(o)),
            Base::Rect(_) => f.write_str(&self.to_string_in_base()),
        }
    }
}

#[derive(Clone)]
enum Shape {
    Circle(Rc<RefCell<Circle>>),
    Rect(Rc<RefCell<Rect>>),
}

impl Shape {
    fn area(&self) -> f64 { match self { Shape::Circle(o) => o.borrow().area(), Shape::Rect(o) => o.borrow().area() } }

    fn describe(&self) -> String {
        match self {
            Shape::Circle(_) => self.describe_in_shape(),
            Shape::Rect(o) => o.borrow().describe(),
        }
    }

    fn describe_in_shape(&self) -> String {
        format!("shape {}", java::Double(self.area()))
    }

    fn to_base(&self) -> Base { match self { Shape::Circle(o) => Base::Circle(o.clone()), Shape::Rect(o) => Base::Rect(o.clone()) } }
}

// Shape.sides() is a static method: a free function `sides()`, as for any class.
```

Rules visible here:

- A dispatcher exists on an enum `T` for every instance method callable on a `T`-typed
  receiver that the program calls on one (or on a subtype enum that forwards to it), including
  `toString`, `equals`, `hashCode` and interface default methods. An arm calls
  `implementation(m, c)` for its variant in the form that class uses today (`o.borrow().m()`,
  `o.borrow_mut().m()`, `C::m(o, ..)`, or `o.m()` for an `Rc<C>` immutable variant), a
  field-level inherited body as `o.borrow().<base_path>.m()`, and an object-level inherited
  body on another enum `S` as `self.to_s().m_in_s()` (or `.m()` when `S` has no dispatcher for
  it). When every arm would be the same forwarder (one target), the dispatcher is that one
  line without a `match`.
- A field read on an enum-typed receiver is `b.base().<path>.name` where `base()` yields the
  fields struct of `fields_root(T)` and `<path>` is `base_path` from there to the declaring
  class (empty here); a write is `b.base_mut().name = v`. On a concrete receiver it is
  `c.borrow().base.name` (or `self.base.name`, `c.base.name` for plain/immutable forms).
- `super.m(args)` is a non-virtual call of the body declared in the superclass `D`:
  field-level `self.base.<path>.m(args)` (or `this.borrow().base...` when `this` is a handle);
  object-level `D::C(this.clone()).m_in_d(args)` (`.m(args)` when `D` has no dispatcher for
  it; when `D` is single-concrete, `C::m_in_d(this, args)` or `self.m_in_d(args)` in the same
  `impl`). A method of a leaf class that calls an object-level `super.m()` therefore uses
  `this` as a value (it is not a `&self` method).
- An abstract method is not emitted anywhere; its dispatcher is.
- A method of an interface or abstract class that nothing calls dynamically or statically is
  still emitted when reachable (single-file mode lowers every method), as today.

### 1.3 Places typed by the base, the interface and Object

```java
class Registry {
    Base head;           // may be null (unset by a constructor)
    Shape[] shapes = new Shape[4];
    Object tag = "none";
    static double total(Base b, Shape s, Object o) { return b.area() + s.area() + (o == null ? 0 : 1); }
}
```

```rust
struct Registry {
    head: Option<Base>,
    shapes: java::Array<Option<Shape>>,
    tag: Object,
}

fn total(b: &Base, s: &Shape, o: Option<Object>) -> f64 {
    b.area() + s.area() + if o.is_none() { 0.0 } else { 1.0 }
}
```

`Base`, `Shape` and `Object` are the enums; nullness, `Option`, borrowed parameters
(`&Base` for a parameter only used through, `borrowed_objects`) and moves follow today's rules
with `Type::Class(name)` of an enum type and `Type::Object` treated like any object type. A
caller converts: `total(&Base::Circle(c.clone()), &s, Some(Object::String(String::from("x"))))`.
`&Base::Circle(c.clone())` is a borrow of a temporary, valid for the call.

### 1.4 Constructors and `super(...)`

A constructor of a class with a superclass starts, in Java, with `super(args)` (written or
implicit `super()`), then runs its own field initializers, then its body. The struct literal
form already in `codegen/owned.rs::struct_literal` takes it in as the first entry:

```rust
fn new(r: f64) -> Circle {
    Circle { base: BaseFields::new("circle"), r }
}
```

- The `base:` value is `<Super>Fields::new_<overload>(args)`, the superclass constructor that
  javac resolved. When the superclass declares no instance fields there is no `base` entry;
  if its constructor has effects (it prints, or calls something), it is still emitted as a
  function `fn new(..)` returning `()` on the superclass's enum-or-impl and called first in the
  body: `BaseFields`-less classes with constructor effects get `Base::init_new(args);` as the
  first statement. (Rare; keep it simple: a unit-returning associated function `new` on the
  enum, or on the single concrete's `impl` named `new_in_<class>`.)
- Ordering: `base` is field 0 of the struct and the first thing Java runs, so the existing
  logic of `struct_literal` (bind values with effects to locals when the literal's field order
  would reorder them) is correct with `base` prepended to `fields`. The leading-assignments
  rule (`analysis::leading_assignments`) skips the `super(...)` statement, which lowering
  represents as `ExprKind::SuperInit { args, resolved }` as the first statement (like
  `ThisCall`).
- A leading assignment may read fields that earlier leading assignments set, as `EnumMap`'s
  constructor does (`keyUniverse = getKeyUniverse(keyType); vals = new
  Object[keyUniverse.length]`): the literal then computes the initializers with effects or that
  read a static first, as Java runs them before the body, and then each leading value into a
  local that the later ones read in place of the field. Every field is set before anything can
  read it, so the nullness analysis sees those fields as never null.
- The superclass's constructor builds a plain value (`plain_constructors`) wherever it can: it
  uses `this` only for the struct being built, calls on `this` only methods that borrow that
  struct, and so do its class's field initializers. A constructor of a class with subclasses
  that uses `this` as a value or calls a method with overriders or an abstract one on `this`
  cannot: Java runs the subclass's override on the object while it is being built, and the
  object does not exist yet when its fields struct is a value of its own. Such a class tree
  gets the second constructor form below.
- The second constructor form (`analysis/whole_objects.rs`, before the analyses): the object is
  allocated first, with every field zero, null or false, then the superclass's constructor body
  runs on the whole object, then the subclass's, as Java runs them. It applies to the classes
  with subclasses (with fields structs) of the class trees where a constructor needs it, and to
  their direct subclasses without subclasses; every other class keeps the form above, so code
  that translated before translates the same. In those trees:
  - each constructor of a class with subclasses becomes an instance method
    `construct_<class>`, whose `super(..)` or `this(..)` is a call of the matching
    `construct_<class>` on `this`, followed (unless it calls `this(..)`) by the class's field
    initializers, then its body; the class gets one constructor without parameters that builds
    the zero fields struct;
  - a constructor of a class without subclasses builds its struct with the zero `base`, sets its
    enclosing instance and captured locals (javac sets them before `super(..)` too), calls the
    superclass's `construct_<class>` with the arguments of its `super(..)`, runs its own field
    initializers, then its body;
  - `new C(..)` of a class with subclasses that has objects of its own calls a function
    `new_<class>` that builds the zero object and calls its `construct_<class>`.

  The field initializers of these classes run as statements after the superclass's part (in a
  method `initialize_fields` when several constructors run them), so a method that a superclass's
  constructor calls sees the subclass's fields zero, null or false, as in Java; an object field
  is then an `Option`. The rewrite is plain Java (constructors split into an allocation and
  methods), so the analyses and the generator need nothing new:

  ```rust
  impl Sorted {
      fn new(previous: Stage) -> Rc<RefCell<Sorted>> {
          let obj = Rc::new(RefCell::new(Sorted { base: StageFields::new(), limit: 0 }));
          let this = &obj;
          Stage::Sorted(this.clone()).construct_stage(Some(previous));
          this.borrow_mut().limit = 7;
          obj
      }
  }

  impl Stage {
      fn construct_stage(&self, previous: Option<Stage>) {
          self.base_mut().previous = previous.clone();
          if let Some(previous) = previous.as_ref() {
              previous.base_mut().next = Some(self.clone());
          }
          self.base_mut().stateful = self.is_stateful();
      }
  }
  ```

  A class with subclasses that has an enclosing instance or captured locals and needs this form
  is still rejected: its synthetic fields would have to be set before the superclass's part.
- A class whose own constructors use `this` as a value keeps today's non-plain form
  (`codegen/objects.rs::constructor`): `let obj = Rc::new(RefCell::new(Circle { base:
  BaseFields::new("circle"), r: 0.0 }));` then `field_inits`, then the body on `this`.
- `this(...)` delegation is unchanged; `super(...)` and `this(...)` are exclusive in Java.
- Statements before `super(...)` or `this(...)` (Java 25's flexible constructor bodies, which the
  JDK's `SortedOps` uses; `lower/prologues.rs`) are written before the object is built, so their
  locals reach the call's arguments and the rest of the body. They cannot use `this`, except to
  assign a field without an initializer: that value goes into a local, `early$f`, and
  `this.f = early$f` right after the call stores it, which the struct literal takes in. In the
  second form above, the stores run on the zero object before the superclass's
  `construct<Name>`, which may call a method that reads them, as in Java. Before a `super()` of
  `Object`, which is left out, the statements are the start of the body; a class with field
  initializers is rejected there, as they would run first.
- Static initialization order: `init_circle()` calls `init_base()` first (section 2.5).

### 1.5 `instanceof` with and without pattern, downcasts

```java
static void show(Shape s, Object o) {
    if (s instanceof Circle c) System.out.println(c.area());
    if (s instanceof Base) System.out.println("base");
    Circle c2 = (Circle) s;                 // ClassCastException when s is a Rect
    Base b = (Base) o;                      // from Object
    if (!(o instanceof Rect)) return;
    Rect r = (Rect) o;
    System.out.println(r.w);
}
```

```rust
fn show(s: &Shape, o: &Object) {
    if let Shape::Circle(c) = s {
        java::println!("{}", c.borrow().area());
    }
    if matches!(s, Shape::Circle(_) | Shape::Rect(_)) {
        java::println!("base");
    }
    let c2 = s.to_circle();
    let b = o.to_base();
    let Object::Rect(r) = o else {
        return;
    };
    java::println!("{}", r.borrow().w);
}
```

Generated conversions (`impl Shape`): `fn to_circle(&self) -> Rc<RefCell<Circle>> { match self
{ Shape::Circle(o) => o.clone(), _ => java::cast_failed("Rect", "Circle") } }` where the first
argument is the name of the variant matched by `_`... since a `match` cannot name it, the
failing arm is `other => java::cast_failed(other.class_name(), "Circle")` with a generated
`fn class_name(&self) -> &'static str` on each enum (Java's binary name per variant). A
downcast to an enum type `D` (`(Base) o`) is `fn to_base(&self) -> Base` with one arm per
common variant and the failing arm; `as_base(&self) -> Option<Base>` is generated when a
pattern binds to an enum type (`o instanceof Base b` is `if let Some(b) = o.as_base()`).
Upcasts from a concrete class are written inline: `Base::Circle(c.clone())` (moved, not
cloned, when the read is a last use per `moves`). `java::cast_failed(from: &str, to: &str) -> !`
goes to the runtime: `throw("java.lang.ClassCastException", &format!("class {from} cannot be
cast to class {to}"))`, exit code 1 like today's runtime failures; the exceptions agent later
turns it into an `Err`.

`instanceof` codegen: with a nullable operand, `matches!(x.as_ref(), Some(Shape::Circle(_)))`
and `if let Some(Shape::Circle(c)) = x.as_ref()`; against a type that shares no variant with
the operand's enum (or a concrete class that is owned), `false` after evaluating the operand
for effects (`{ let _ = expr; false }` only when the operand has effects; a variable just gives
`false`); when the operand's static type is a subtype of the tested type: `true`
(`x.is_some()` if nullable). The negated-early-exit form above is recognised as: `Stmt::If {
cond: !InstanceOf(Var(v), C, no binding), then: exits, otherwise: None }` followed later in the
same block by `Stmt::Local { init: Cast(C, Var(v)) }` with `v` never assigned in the block, and
the pattern form `if (!(o instanceof Rect r)) return;` (Java's flow scoping) is the same
let-else; the `Local` is then skipped and the binding carries the local's name. A bound
variable is a reference (`&Rc<RefCell<Rect>>` or `&Rc<Rect>`); it goes into `Gen::borrowed`
like a null-narrowed name, so reads write `r.borrow().w` and uses as values write `r.clone()`.
Patterns in `switch` stay rejected ("patterns in switch").

### 1.6 Identity and null

```java
Base b = ...; Base c = ...; Circle k = ...; Base n = null;
b == c; b != k; n == null; n == b;
```

```rust
b == c; b != Base::Circle(k.clone()); n.is_none(); n == Some(b.clone())
```

`impl PartialEq for <enum>` is identity: `Rc::ptr_eq` per variant, `a == b` for Java enum
variants (constants are singletons), `java::same(&a, &b)` for faithful boxed variants, `false`
across variants. `Option<Enum>` compares with `==` (both `None` is `true`, as in Java). A
comparison whose operands have a static type whose variants include `String` or a non-faithful
box is rejected in `analysis::validate` with today's message for strings ("== on strings
compares references; not supported yet") or made faithful for boxes (2.8). Plain handles keep
`Rc::ptr_eq`/`java::same`.

### 1.7 Object's defaults: `toString`, `equals`, `hashCode`; boxing into Object

```java
class Point { int x; }                       // no toString, no equals
class Key { int k; public boolean equals(Object o) { return o instanceof Key other && other.k == k; } public int hashCode() { return k; } }
Object o = 5; Object s = "s"; Object p = new Point();
System.out.println(p);                       // Point@1b6d3586
o.equals(s); p.hashCode(); o.toString();
```

```rust
#[derive(Clone)]
enum Object {
    Point(Rc<RefCell<Point>>),
    Key(Rc<RefCell<Key>>),
    String(String),
    Integer(i32),
}

impl Object {
    fn equals(&self, other: &Object) -> bool {
        match self {
            Object::Point(a) => matches!(other, Object::Point(b) if Rc::ptr_eq(a, b)),
            Object::Key(a) => a.borrow().equals(other),
            Object::String(a) => matches!(other, Object::String(b) if a == b),
            Object::Integer(a) => matches!(other, Object::Integer(b) if a == b),
        }
    }

    fn hash_code(&self) -> i32 {
        match self {
            Object::Point(a) => java::identity_hash(a),
            Object::Key(a) => a.borrow().hash_code(),
            Object::String(s) => java::string_hash(s),
            Object::Integer(i) => *i,
        }
    }

    fn class_name(&self) -> &'static str { match self { Object::Point(_) => "Point", Object::Key(_) => "Key", Object::String(_) => "java.lang.String", Object::Integer(_) => "java.lang.Integer" } }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Object::Point(o) => write!(f, "{}", o.borrow()),
            Object::Key(o) => write!(f, "{}", o.borrow()),
            Object::String(s) => f.write_str(s),
            Object::Integer(i) => write!(f, "{i}"),
        }
    }
}

impl fmt::Display for Point {              // the default toString
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Point@{:x}", java::object_hash(self))
    }
}

impl Key {
    fn equals(&self, o: &Object) -> bool {
        matches!(o, Object::Key(other) if other.borrow().k == self.k)
    }
    fn hash_code(&self) -> i32 { self.k }
}
```

- `java::object_hash<T: ?Sized>(value: &T) -> i32` is the address-based hash; `identity_hash(rc)`
  becomes `object_hash(&**rc)` so a class's default `Display` and `hashCode` agree. The name
  printed is Java's binary name: package-qualified, `$` between nested classes.
- Every class that lacks `toString` gets this `Display`; `check_text` and the test
  `printing_an_object_without_to_string_is_rejected` are replaced (the corpus never prints
  such values, as the text cannot match Java's).
- Every instance `equals(Object)`/`hashCode()` of a program class is an ordinary method
  (`MethodCall` resolved to it). A call `a.equals(b)` resolved to `Object.equals` (the runtime
  method; its index is a synthetic method entry, see milestone 4) dispatches: receiver a concrete class
  with its own `equals` → that; a concrete class without one → `Rc::ptr_eq(a, b_as_handle)` /
  `false` when `b` cannot be that class; an enum → the enum's `equals` dispatcher; the argument
  is converted to `Object` (`&Object::Key(b.clone())`) unless the target's parameter type is
  narrower. `Object.hashCode` likewise (`java::identity_hash(&a)` default). `String.equals`
  with an Object argument stays in `lower.rs`'s string lowering as a one-line addition: the
  argument `Object` → `matches!(arg, Object::String(t) if t == s)` (`Builtin::StringEquals` with
  an `Object`-typed second argument; the generator handles it in `null.rs::equals_optional`).
- `Object o = 5` is `Object::Integer(5)`; `Object s = "s"` is `Object::String(String::from("s"))`;
  a Java enum `Object::Color(Color::Red)`; `(Integer) o` is `o.to_integer()` (throwing), then
  unboxing as today; `(String) o` is `o.to_string_()`? No: name it `o.to_java_string()` to
  avoid `ToString::to_string`; runtime variants' conversion names are `to_java_string`,
  `to_integer`, `to_long`, ...
- Boxed kinds inside `Object` are plain primitives unless an identity comparison exists on a
  type whose variants include that kind: then the kind joins `faithful_boxes` and the variant
  is `Integer(java::Boxed<i32>)` (2.8).
- Arrays into `Object` and `Object[]`/`Object` arrays as values come with milestone 5.

### 1.8 Shared, immutable, owned under inheritance

- `owned` (plain value): as today, and additionally never when the class has subclasses, is
  ever converted to a supertype (any `Cast` with it as the source type, including the
  implicit ones lowering inserts, and `this` converted inside its methods), or any of its
  objects receive a call of an inherited object-level method or `super.m()` of one.
- `immutable` (`Rc<C>`): computed on the whole object (own and inherited fields: a field of
  `BaseFields` written by any non-constructor method, through any receiver, makes every class
  of the family mutable), then unified per family: all concrete classes of a class tree or
  none. Interface enums and `Object` may mix `Rc<C>` and `Rc<RefCell<C>>` variants; a class
  tree's enum therefore has `base()` returning `Ref<'_, XFields>` (RefCell family) or
  `&XFields` (immutable family), and `base_mut()` only for the former.
- A fields struct has no representation of its own: it is always inline inside a concrete
  object. Its methods take `&self`/`&mut self` and are reached through the concrete object's
  form.

### 1.9 A `Comparable` implementor; interfaces outside the sources

```java
class Version implements Comparable<Version> {
    final int major, minor;
    public int compareTo(Version o) { return major != o.major ? Integer.compare(major, o.major) : Integer.compare(minor, o.minor); }
}
```

```rust
struct Version { major: i32, minor: i32 }
impl Version {
    fn compare_to(&self, o: &Rc<Version>) -> i32 { if self.major != o.major { self.major.cmp(&o.major) as i32 } else { self.minor.cmp(&o.minor) as i32 } }
}
```

Nothing else is generated unless the program uses `Comparable` as a type (then
`enum Comparable { Version(Rc<Version>), String(String), ... }` with a `compare_to(&self,
other: &Object) -> i32` dispatcher whose arms convert the argument with `to_version()` etc.,
the bridge javac generates) or as a bound of a real generic (then the trait of 1.11). In
single-file mode `java.lang.Comparable` is a *runtime interface*: `lower/hierarchy.rs` has a
table `RUNTIME_INTERFACES: &[(&str, &[(&str, &str)])]` = `[("java.lang.Comparable",
&[("compareTo", "(T)int")])]`; implementing any other runtime interface (`Runnable`,
`Iterable`, `CharSequence`, ...) is rejected: "implementing `java.lang.Runnable` not supported
yet". In whole-program mode with the JDK sources, `java.lang.Comparable` is an ordinary source
interface and the runtime types are its runtime variants. The dispatch arm for a runtime
variant comes from a table in `codegen/dispatch.rs`: `("java.lang.Comparable", "compareTo")`:
`String` → `java::string_compare_to(a, &b.to_java_string())`, boxes → `a.cmp(&b.to_integer()) as
i32` / `java::double_compare`, Java enums → `a as i32 - b.to_color() as i32`;
`("java.lang.Object", "toString"|"equals"|"hashCode")` as in 1.7. A call of an interface method
on a value that may be a runtime variant without a table entry is rejected in
`analysis::validate`: "calling `Iterable.iterator` on a value that may be a String".

### 1.10 Printing

An enum type implements `fmt::Display` by dispatching per variant to the class's `Display`
(`write!(f, "{}", o.borrow())`), to its plain `to_string` method when the class's `toString` is
not a `Display` (`f.write_str(&C::to_string(o))`), or to the inherited body. `b.toString()`
calls on an enum receiver are `b.to_string()` through `ToString` (no inherent `to_string` on
enums: clippy's `inherent_to_string_shadow_display` forbids it; the inherited body is named
`to_string_in_base`). `quiet_text` (printing may format the object in place) holds for an enum
value when it holds for every variant; otherwise `b.to_string()` is computed first, as today.
`java::or_null` works unchanged for `Option<Enum>`.

### 1.11 A generic class, a generic method, an `ArrayList`-like class

Erased form (milestone 5, always available):

```java
class Box<T> { T value; Box(T v) { value = v; } T get() { return value; } }
class Store<E> {
    Object[] data = new Object[4]; int n;
    void add(E e) { data[n++] = e; }
    @SuppressWarnings("unchecked") E at(int i) { return (E) data[i]; }
}
static <T extends Comparable<T>> T max(T a, T b) { return a.compareTo(b) > 0 ? a : b; }
Box<Integer> bi = new Box<>(5); int v = bi.get();
Store<Shape> st = new Store<>(); st.add(circle); Shape back = st.at(0);
String m = max("a", "b");
```

```rust
struct Box { value: Object }                         // T erases to Object; the constructor sets it, so no Option
impl Box {
    fn new(v: Object) -> Box { Box { value: v } }
    fn get(&self) -> Object { self.value.clone() }
}
struct Store { data: java::Array<Option<Object>>, n: i32 }
impl Store {
    fn add(&mut self, e: Object) { let i = self.n; self.n = self.n.wrapping_add(1); java::set(&self.data, i, Some(e)); }
    fn at(&self, i: i32) -> Option<Object> { java::get(&self.data, i) }    // (E) is erased to (Object): no check
}
fn max(a: Comparable, b: Comparable) -> Comparable { if a.compare_to(&b.to_object()) > 0 { a } else { b } }

let bi = Rc::new(RefCell::new(Box::new(Object::Integer(5))));
let v = bi.borrow().get().to_integer();
let st = Rc::new(RefCell::new(Store::new()));
st.borrow_mut().add(Object::Circle(circle.clone()));
let back = java::non_null(st.borrow().at(0)).to_shape();
let m = max(Comparable::String(String::from("a")), Comparable::String(String::from("b"))).to_java_string();
```

Real form (milestone 6, when its conditions hold; `Box` and `max` qualify, `Store`
does not because of `Object[]` and the unchecked cast):

```rust
struct Box<T> { value: T }
impl<T: java::Value> Box<T> {
    fn new(v: T) -> Box<T> { Box { value: v } }
    fn get(&self) -> T { self.value.clone() }
}
fn max<T: Comparable<T>>(a: T, b: T) -> T { if a.compare_to(&b) > 0 { a } else { b } }

trait Comparable<T> { fn compare_to(&self, other: &T) -> i32; }
impl Comparable<String> for String { fn compare_to(&self, other: &String) -> i32 { java::string_compare_to(self, other) } }
impl Comparable<i32> for i32 { fn compare_to(&self, other: &i32) -> i32 { self.cmp(other) as i32 } }
impl Comparable<Rc<Version>> for Rc<Version> { fn compare_to(&self, other: &Rc<Version>) -> i32 { Version::compare_to(self, other) } }

let bi = Rc::new(RefCell::new(Box::new(5)));
let v: i32 = bi.borrow().get();
let m = max(String::from("a"), String::from("b"));
```

`java::Value` is a runtime trait, `pub trait Value: Clone { fn to_text(&self) -> String; fn
hash_code(&self) -> i32; fn equals(&self, other: &Self) -> bool; fn same(&self, other: &Self)
-> bool; }`, implemented in the runtime for `String`, every primitive box type (`i32`, `i64`,
`f64`, `f32`, `i16`, `i8`, `u16`, `bool`), `Boxed<T>` and `Array<T>`, and generated for every
program class representation and enum that is ever a type argument of a real generic
(`impl java::Value for Rc<RefCell<Point>> { fn to_text(&self) -> String { self.borrow().to_string() } fn hash_code(&self) -> i32 { java::identity_hash(self) } fn equals(&self, other: &Self) -> bool { Rc::ptr_eq(self, other) } fn same(..) }`,
calling the class's own `equals`/`hashCode` when it has them, through `Object::Point(other.clone())`).
Inside a real generic, `"" + t` is `t.to_text()`, `t == u` is `t.same(&u)`, `t.equals(u)` with
`u: T` is `t.equals(&u)`, `t.hashCode()` is `t.hash_code()`; `t.compareTo(u)` comes from the
bound's trait. Where the type arguments differ in representation (`Box<Circle>` holds
`Rc<RefCell<Circle>>`, `Box<Shape>` holds the `Shape` enum) Rust's monomorphization does the
work; the variant of a real generic class in an enum is per instantiation,
`Shape::BoxOfInteger(Rc<RefCell<Box<i32>>>)`.

Where the two forms meet: javac's `javaType` on every use-site expression is the
*substituted* type (`Integer` for `bi.get()`), while the declared type of the member is the
type variable. Lowering gives the call/field expression the declared type (`Type::Param`) and
wraps it in `Cast(substituted, expr)`; arguments and stores get `Cast(declared, arg)`. The
generator decides per `Cast`: when the `Param` belongs to a real generic, the cast is a
no-op (Rust substitutes); when it belongs to an erased one, it is the `to_x()` conversion
from the erasure (`Object`, or the bound's enum), or the variant wrap into it. This is exactly
javac's checkcast placement, so erased code is right even for raw types, wildcards
(`capture#1 of ? extends Shape` reads as `Shape`, `? super X`/`?` as `Object`) and generic
arrays (`E[]` is `Object[]`, that is `java::Array<Option<Object>>`).

### 1.12 Interfaces: default and static methods, constants, enums implementing interfaces

Shown in 1.2 (`describe`, `sides`). Interface constants are `static final` fields: `const`
items or cells exactly like a class's statics. A Java enum implementing an interface is a
variant holding the Copy value: `Shape::Color(Color)`, and its arm calls the enum's method
`c.area()` (enum methods take `self` by value). An interface extending interfaces: its enum's
variants are the union over its concretes as for any type; `to_<super interface>()`
conversions are generated on demand. A class implementing two interfaces appears in both
enums. Interface methods are always object-level (interfaces have no instance fields).

A class may implement an interface's method with a method it inherits from a superclass that
does not implement the interface (`Spliterators.EmptySpliterator.OfInt`): javac's `overrides`
of the superclass's method cannot list it, so `JavaTree` gives the class's symbol an `inherits`
list of such pairs, and reachability and lowering add the interface's method to the overrides of
the inherited one (with a bridge when erasure changes its signature). An interface that declares
`Object`'s `equals` or `hashCode` again (`Comparator`) dispatches a class or enum without its
own to `Object`'s.

### 1.13 Dropping long chains

Objects are freed by reference counting, and Rust frees an object's fields when it frees the
object. Left alone, freeing the head of a linked list of a million nodes would free its `next`,
which frees its `next`, a million frames deep, and the program would abort with a stack
overflow on the 8 MB main stack. Java's collector has no such limit, so the translation must
not either. The rule (`codegen/chains.rs`, runtime `chains.rs`):

- **Which classes.** The *held classes* of a field type are the concrete classes whose objects
  a value of it holds directly: the class an object type stands for, the variants of the enum
  of a class type (program classes only, not strings, boxes or Java enums), and for an array
  the held classes of its element type. A type variable of a real Rust generic holds none
  (its instantiations are not followed). Concrete class `c` has an edge to each held class of
  each of its fields, its own and those of the fields structs it embeds. A field of a struct
  is a *link* when, for some concrete class whose objects have the field, one of its held
  classes is that class or can reach it by edges: the field can lead back to an object of the
  same class, directly (`Node next`) or through a cycle of classes (`Outer.inner` and
  `Inner.outer`), and so a chain of such objects can be as long as the program makes it.
- **What they get.** A struct with links gets `impl java::Unlink<'a>`, whose `unlink` calls
  `java::Link::unlink_in(&mut self.f, chain)` for each link (and `unlink` of its embedded
  `base` when that struct has links), and `impl Drop` calling `java::drop_links(self)`. A
  struct without links of its own whose embedded fields struct has them gets only the
  `Unlink`, which unlinks `base`; the fields struct's own `Drop` runs when the struct's
  fields are dropped. The enum of a class type that is the type (or the array element type) of
  a link gets `impl java::Link<'a>`, which matches the variants whose structs unlink and does
  nothing for the others.
- **How it frees.** `drop_links` moves the links out of the dying object onto a
  `java::Chain`, a stack of `Rc<dyn Unlink>` on the heap, and frees the objects on it one at a
  time, unlinking each before it is freed, so each object dies with its links already gone and
  the drop depth stays constant. `Link` is implemented in the runtime for every form a link
  has: `Option` (taken out), `Rc` of an object (pushed onto the chain), `Vec` and so
  `java::Array` (the elements are drained onto the chain) and the generated enums. An object
  goes on the chain only while the chain holds its last reference; any other reference keeps
  it alive, and dropping the chain's only decrements the count, so a list that is still
  reachable from somewhere else is not touched. A link that cannot be null (a plain `Rc`
  field) cannot be taken out; when nothing else holds its object, the object is unlinked in
  place. A cycle of non-null links cannot end, so such an object is on a reference cycle and
  never freed, and the in-place unlinking goes at most one class cycle deep.
- **Shallow structures stay recursive.** Rust's own recursive drop is the fastest way to free
  a structure, and most structures are shallow (a balanced tree of a million nodes is 20
  deep). A thread-local holds the highest stack address any `drop_links` on the thread has run
  at. A `drop_links` that runs less than 128 KB of stack below it does nothing, and Rust drops
  the fields recursively as it would without the `Drop`; one deeper than that moves the links
  onto a chain, which frees the rest of the structure iteratively at that depth. So the drops
  of a structure use at most 128 KB of stack below the shallowest drop of the thread, plus
  constant space.
- **What it costs.** Only the classes on cycles of field types change. Each object of such a
  class that dies runs its `Drop`: near the top, one thread-local read and a compare; deeper,
  the takes of its links and a heap stack that is allocated only when a link held the last
  reference to its object. The binary trees benchmark, which frees millions of small trees, is the worst case;
  its numbers are in docs/performance.md.
- **Not covered.** A chain through a field whose type is a type variable of a real Rust
  generic (`Box<T>` holding `Box<Box<...>>`), or through objects captured by a lambda. Reference
  cycles are still never freed, as before.

## 2. Interaction with the existing analyses

Where a change is "every site that matches `resolved: Some(callee)`", the sites are:
`analysis.rs` (13: `borrowed_objects`, `used_as_value`, `ascii_strings`), `analysis/nullness.rs`
(6), `analysis/borrowing.rs` (3), `analysis/ranges.rs` (2), `analysis/arrays.rs` (4),
`analysis/statics.rs` (4), `codegen/objects.rs` (6), `codegen/owned.rs` (1),
`codegen/strings.rs` (2). Add `ast::Class::callees(&self, e: &Expr) -> Vec<usize>` returning
`hierarchy.targets(resolved, receiver static type)` for a `MethodCall`, `{resolved}` for
`Call`/`New`/`ThisCall`/`SuperInit`/`SuperCall`, and use it at every site so that a call "runs
any overrider".

### 2.1 Nullness (`analysis/nullness.rs`)

- `nullable_fields` is keyed by the *declaring* class: `Place::Field`/`ExprKind::Field` gain
  `owner: usize` (set by lowering from the symbol's owner) and the key becomes
  `(types[owner].name, field)`; `field_nullable(owner, name)` in codegen takes the owner index.
  Today's key by `object.ty` is wrong once the receiver may be a subclass or an enum.
- A call's argument flows into parameter `i` of every method in `group(callee)` (so a
  dispatcher and all its targets agree on `Option` or not); a call's result may be null if any
  member of the group has a nullable return. This happens inside the fixpoint where
  `params.push((*callee, i))` and `facts.nullable_returns.contains(callee)` are today.
- `maybe_null(Cast(_, v))` already passes through; `InstanceOf` is never null (a boolean);
  a pattern binding is never null; `SuperInit`/`SuperCall` behave like `ThisCall`/`MethodCall`.
- A field of an enum type or `Object` follows `may_be_unset` (add `Type::Object`,
  `Type::Param`, `Type::Generic` to it and to `is_reference`).
- `set_by_construction` for a field declared in `D` checks the constructors of `D` (the
  fields struct's constructors), not of the concrete class: inherited fields are set by the
  superclass constructor the subclass's literal embeds.

### 2.2 Borrowing and ownership (`analysis/objects.rs`, `owned.rs`, `borrowing.rs`)

- `objects.rs::is_immutable(t)`: a write to field `f` through an object whose static type is
  `S` counts against the class `field_owner(S, f)` and so against every concrete class whose
  chain includes it; after the per-class result, `immutable_classes` intersects over each
  family (2.8 of section 1): a family is immutable only if all its members are. Fields structs
  are "immutable" iff their family is (the generator asks `immutable.contains(concrete)`).
- `owned.rs::is_owned(t)`: add the three rules of 1.8. "Ever converted" is any
  `ExprKind::Cast(to, e)` with `e.ty == Class(t)` and `to != e.ty` anywhere, including the
  casts lowering inserts and a `Cast` of `this` inside `t`'s methods; "inherited object-level
  call" is a `MethodCall` on a `t`-typed receiver whose `implementation(m, t)` is object-level
  and declared above `t`, or any `SuperCall` of an object-level method in `t`'s methods.
- `borrowing.rs`: `Scan::touch(ty)` inserts every concrete class of `concretes(ty)` (for an
  enum type, all its variants' classes; for `Object`, every class); `class_of(m)` for a method
  declared in `D` is the set `concretes(D)` and the test `reach[m].contains(class_of(m))`
  becomes "intersects"; `Scan.calls` pushes every target of `callees(e)` with `on_this`; `this`
  is a value (`this_as_value`) also when it is the source of a `Cast`, the receiver of a call
  whose implementation is object-level and declared in a type with an enum (it must be wrapped
  into the enum), or the receiver of an object-level `SuperCall`. `writes` of a field write
  through an enum-typed object inserts all its concretes. `mutating` of a dispatcher is the
  union over its targets (the dispatcher takes `&self` of the enum and borrows inside, so the
  generator treats a dispatcher call like a call of a non-`self` method: `borrows_here` counts
  `(1, writes)`); `writers`/`isolated` propagate through `callees`.
- `plain_constructors` must hold for every class with subclasses once the class trees that need
  it have the second constructor form of 1.4 (`analysis::validate` checks it).
- `self_methods` for a field-level method of `DFields`: the same test as for a leaf class's
  method, with `touches` compared against the family's concretes. An object-level method on an
  enum is never a `self_method` in today's sense (its `self` is the enum, `ThisForm::Enum`).

### 2.3 `writers`, `mutating`, `isolated` through dispatch

A call resolved to `m` on a receiver of static type `S` may run any of `targets(m, S)`:
`writers` and `isolated` propagate over all of them, and `conflicts`/`borrows_here` in
`codegen/objects.rs` ask `writers` for the dispatcher's method index (which the analysis sets
to the union). `changes_object` (used for `mut` bindings and argument ordering) asks
`mutating` of any target.

### 2.4 `borrowed_objects` and the other parameter forms

Parameter forms (`borrowed_params`, `borrowed_optional_params`, `borrowed_objects`,
`borrowed_arrays`, `nullable_vars` of parameters, `nullable_returns`) are unified over each
override group after they are computed: `analysis::align_groups(class, facts)` removes a
form from every member unless every member has it (and makes a nullable parameter/return
nullable in every member). Dispatchers then forward arguments unchanged. An enum-typed
parameter used only through is `&Base` (borrowed), like `&Rc<..>` today; `object_ref` of a
concrete value for such a parameter writes `&Base::Circle(c.clone())`.

### 2.5 Statics and initialization order (`analysis/statics.rs`, `program/reach.rs`, `codegen/statics.rs`)

- `reach.rs::init(class)` pushes `Item::Init(superclass)` and `Item::Init(interface)` for
  interfaces declaring default methods (JLS 12.4.1); the comment there already anticipates it.
- `codegen/statics.rs::init_guard` is followed by `init_<superclass>();` when the superclass
  is eager; `entry_inits` makes a constructor or static method run the init of the nearest
  eager class at or above its own (a subclass constructor is the first use of an eager
  superclass): `Facts::entry_init` becomes `HashMap<usize, usize>` (method → class to init).
- Interface constants are statics of the interface's type index; nothing else changes.

### 2.6 Struct-literal constructors (`analysis.rs::leading_assignments`, `codegen/owned.rs`)

`leading_assignments` skips a leading `SuperInit`; `inline_initializers` of a class also
requires that its superclass's constructors are plain. `struct_literal` prepends the `base`
entry (value: the super constructor call with `call_args`, or the default `SuperFields::new()`
for an implicit `super()`), counts it among `effects` first, and never binds it to a local
unless a later initializer with effects precedes it in the literal (impossible, it is first).
`field_locals` and `reads_earlier_fields` see inherited fields as `this.base.x` reads: an
initializer of `Circle` reading an inherited field reads `base.name` from the literal's `base`
value, which is already computed: allowed (the struct literal binds `base` first, as
`let base = BaseFields::new("circle");` when another initializer reads it).

### 2.7 Enums (Java), boxing, printing

- Java enums: `TypeDecl.kind == Enum`, unchanged representation; they get `implements`;
  `Type::Enum` values convert into interface and `Object` enums by variant wrap, and
  `Comparable` dispatch uses ordinals. `enum_items` is unchanged except for the `Display` that
  the interface enum's arm uses.
- Boxing: `boxing.rs::analyze` adds kind `k` to `faithful_boxes` when a `==`/`!=` compares
  values of a type whose variants include `Boxed(k)` (2.8); `Object::Integer` then holds
  `java::Boxed<i32>` and `to_integer()` returns it (unboxing as today).
- Printing: `converted_objects`, `object_text`, `to_string_index`, `displays`, `quiet_text` in
  `codegen/objects.rs` work on `Type::Class` of enum types and `Type::Object` through the
  enum's `Display` (1.10); `to_string_index` for an enum receiver is the `toString` dispatcher
  entry (a synthetic method, milestone 4), for a class without `toString` the synthetic default (milestone 4).

### 2.8 Facts added

```rust
pub struct Facts {
    // as today, plus
    /// Enum types (indexes into `Class::types`, plus `OBJECT` = usize::MAX for java.lang.Object) and their variants.
    pub variants: HashMap<usize, Vec<Variant>>,
    /// Types with exactly one concrete class, mapped to it.
    pub single: HashMap<usize, usize>,
    /// Instance methods of non-leaf types that need the whole object (see "object-level").
    pub object_level: HashSet<usize>,
    /// Methods that have a dispatcher on their declaring type's enum (some variant overrides them).
    pub dispatched: HashSet<usize>,
    /// Conversions the program needs, as (from type, to type): the `to_x`/`as_x` methods to generate.
    pub conversions: BTreeSet<(Type, Type, bool /* optional */)>,
    /// Generic classes and methods translated as real Rust generics (milestone 6); the rest are erased.
    pub real_generics: HashSet<usize>,
    /// Interfaces generated as traits because a real generic bounds a type parameter with them.
    pub traits: HashSet<usize>,
    /// Types whose representation implements `java::Value` (type arguments of real generics).
    pub values: BTreeSet<Type>,
    pub entry_init: HashMap<usize, usize>,   // was HashSet<usize>
}
pub enum Variant { Class(usize), String, Boxed(Type), Enum(usize), Array(Type), Instance(usize, Vec<Type>) }
```

Computed in a new `analysis/hierarchy.rs` (`variants`, `single`, `object_level`, `dispatched`,
`conversions`, families) before `objects::immutable_classes` and `owned::owned_classes` in
`analyze`, since those consult conversions; `owned` is computed before `variants` (variants
exclude owned classes), so: conversions → owned → variants → immutable (family-unified) →
borrowing → ... A new `analysis::validate(class, &facts) -> Result<(), Error>` runs after
`analyze` in `lib.rs::translate` and `program.rs::translate_program`, holding the rejections
that depend on facts (1.4's constructor rule, 1.6's string identity, 1.9's runtime variant
without a dispatch entry, milestone 6's generic conditions never reject: the erased form is the fallback).

## 3. Milestones

Each milestone is mergeable on its own with `cargo fmt`, `cargo clippy --all-targets -q --
-D warnings`, `cargo test -q` green, a commit, and `scripts/check.sh` printing "check passed".
Each lists what lowering newly accepts, the AST, analysis, codegen and runtime changes, what
stays rejected (with the message), the tests to add (shape tests in
`crates/translator/tests/translation.rs`, corpus programs by path), the survey change, and the
README paragraph to update ("Current state" lists supported features; keep it accurate).

### Milestone 1: superclasses with one concrete class (embedded fields, super, abstract classes)

Accepts `class C extends D` where every type with subclasses has exactly one concrete class
(the "abstract base and its one implementation" shape, and chains like `Entity ← Account`
where `Entity` is never instantiated). No enums yet.

- **Lowering** (`lower/objects.rs::declare_type`, new `lower/hierarchy.rs`): accept
  `extendsClause` naming a program class (not an interface, not a runtime class other than
  `Object`), `ABSTRACT` flag on classes and methods, methods without a body when abstract;
  resolve `extends` to indexes after all types are declared (two passes: declare names, then
  headers); fill `Method.overrides` from the symbol's `overrides` list (owner + name +
  signature → `method_index`); `super(args)` as `ExprKind::SuperInit { args, resolved }` (the
  first statement; today `is_super_call` drops `super()` — keep dropping it only when the
  superclass is `Object`); `super.m(args)` as `ExprKind::SuperCall { name, args, resolved }`;
  field symbols whose owner is an ancestor resolve to `Field { object, name, owner }`; implicit
  reference conversions inserted by a helper `convert(expr, to: Type) -> Expr` at: `Stmt::Local`
  init, `Assign` value (to the place's type), call/new/this/super arguments (to parameter
  types), `Return` (to the method's return type), `ArrayLit` items and `Index` stores (to the
  element type), `Conditional` arms (to the expression's type), field initializers. `ty()` maps a
  class name to `Type::Class` whether abstract or not. Reject in this milestone: a type with
  two or more concrete classes ("class hierarchies where `Base` has several concrete classes
  not supported yet"), `implements`, interfaces, generics, `instanceof`, explicit casts
  between class types, `Object` as a type (all with today's messages), a superclass outside
  the sources other than `Object` ("extending `java.lang.Thread` not supported yet"), a
  constructor of a class with subclasses that uses `this` as a value (1.4).
- **AST**: `TypeKind`, `is_abstract`, `extends`, `implements` (empty), `Method::{is_abstract,
  overrides}`, `Hierarchy`, `ExprKind::{SuperInit, SuperCall}`, `owner` on `Field`/`Place::Field`;
  `visit_expr`/`visit_place`/`has_effects`/`moves.rs::expr` handle the new kinds.
- **Analysis**: everything in section 2 except variants/dispatch: `callees`, group alignment,
  field-owner keys, family immutability (a family is one tree here), owned rules, `touch`
  over concretes, statics order, `leading_assignments` with `SuperInit`, `object_level`
  classification (needed to place a superclass's methods: field-level on `DFields`,
  object-level in the concrete's `impl`), `validate`.
- **Codegen** (`codegen/objects.rs::type_items`, new `codegen/hierarchy.rs`): emit
  `<D>Fields` structs and their `impl` (constructors as plain-value builders, `field_inits`,
  field-level methods) for every class with subclasses that has fields, in `types` order;
  concrete structs with `base`; field paths through `base_path`; `SuperInit` in
  `struct_literal`/`constructor`; `SuperCall`; a superclass's object-level methods emitted in
  the concrete class's `impl` (`this_form` as for its own methods; names `m` or `m_in_d` when
  overridden); `live_types` marks a fields struct live when a live class embeds it;
  `method_names` allocates names per emitted `impl` (fields struct or concrete class), with
  `_in_d` suffixes.
- **Runtime**: nothing.
- **Tests**: shape tests `superclass_fields_are_embedded` (expects `struct Circle {\n    base:
  BaseFields,` and `Circle { base: BaseFields::new("circle"), r }`),
  `inherited_field_methods_live_on_the_fields_struct` (`impl BaseFields {` with `fn
  get_name(&mut self)`, call `c.borrow_mut().base.get_name()`),
  `super_calls_are_non_virtual` (`fn to_string_in_base`), `abstract_methods_are_not_emitted`,
  `a_subclass_constructor_initializes_the_superclass_first` (`init_base();` as the first line
  of `init_circle`), `a_superclass_constructor_that_leaks_this_is_rejected`. Corpus:
  `corpus/features/Inheritance.java` (abstract base, one subclass, inherited fields and methods,
  super constructor with arguments, overriding with `super.m()`), `corpus/features/SuperInit.java`
  (field initializers in both classes with side effects, printing order, `this(...)` plus
  `super(...)`), `corpus/programs/Ledger.java` (an `Entry` base with `Deposit` as its only
  concrete class, a registry of entries, static init in both classes),
  `corpus/projects/accounts` (two packages, the base in one and the subclass in another).
- **Survey**: no feature removed yet (inheritance is not complete); the "only blocker"
  column is unaffected.

### Milestone 2: several concrete classes: enums, dispatch, identity, display

Accepts any class tree of program classes (abstract or not, any depth).

- **Lowering**: lift the "several concrete classes" rejection; `Type::Class(name)` of a
  class with subclasses means the enum when it is not single-concrete (the generator decides
  per `Facts::single`).
- **Analysis** (`analysis/hierarchy.rs`): `variants` (program classes only in this
  milestone), `single`, `dispatched`, `conversions` (every `Cast` from a class type to a class
  type, both directions, and the `(enum, enum)` pairs needed by dispatcher arms that forward to
  an inherited body on another enum), family immutability over an enum's variants, 2.3 and
  2.4. `validate`: nothing new.
- **Codegen** (`codegen/hierarchy.rs`): per enum type: `#[derive(Clone)] enum`, `base()`/
  `base_mut()` (only when `fields_root` exists; `&XFields` form for immutable families),
  dispatchers and forwarders for every method called on a receiver of that type (including
  those reached through `to_x()` forwarding), inherited object-level bodies (`m_in_d`),
  `class_name()`, `impl PartialEq` (identity), `impl fmt::Display`, the `to_x()` conversions
  in `conversions` (enum→enum upcasts and downcasts; concrete→enum is inline). New
  `ThisForm::Enum`: `this` is `self: &Enum`; `object_parts`/`field_read`/`field_effect`/
  `update` write `self.base().x`/`self.base_mut().x = v`, `this` as a value is
  `self.clone()`, identity `*self == other`. `Cast` codegen (upcasts only in this milestone;
  explicit downcasts come in milestone 4 but the machinery is the same `to_x()`). `identity`
  in `objects.rs` handles enum operands (`==` on the enums, upcasting a concrete operand);
  `null_test` unchanged (`is_none`). `handle_text`/`object_text`/`quiet_text` over variants.
  Dispatcher arms convert arguments and results between the group's unified declared types
  and the target's own (covariant returns upcast by variant wrap; parameter narrowing waits
  for milestone 5's bridges).
- **Runtime**: nothing.
- **Tests**: shape tests `a_type_with_several_concrete_classes_is_an_enum` (expects
  `enum Base {\n    Circle(Rc<RefCell<Circle>>),\n    Rect(Rc<RefCell<Rect>>),\n}`, the `area`
  dispatcher text exactly as in 1.2, `fn base(&self) -> Ref<'_, BaseFields>`),
  `inherited_bodies_that_dispatch_live_on_the_enum` (`fn to_string_in_base(&self)`),
  `upcasts_wrap_the_handle` (`Base::Circle(c.clone())`, and a moved last use
  `Base::Circle(c)`), `enum_identity_is_pointer_equality` (`impl PartialEq for Base`, `b == c`),
  `an_immutable_family_has_no_refcell` (`enum N {\n    Leaf(Rc<Leaf>),` and `fn base(&self) ->
  &NFields`), `a_class_that_is_never_upcast_stays_a_plain_value_beside_its_siblings`,
  `a_class_that_is_upcast_is_shared`. Corpus: `corpus/features/Dispatch.java` (two and three
  level trees, overrides at each level, `super.m()` chains, a method inherited unchanged by
  some classes and overridden by others), `corpus/features/Identity.java` (`==` across
  variants, with null, after upcasts), `corpus/features/Families.java` (an immutable family
  and a mutable one, a mutable write through the base type), `corpus/programs/Shapes.java` (the
  shapes example without `List`, `String.format` and `getClass`: an array of `Base`, totals,
  printing), `corpus/programs/Expressions.java` (an expression tree `Node ← Num | Add | Mul`
  with `eval()` and `toString()`, built recursively and printed), `corpus/projects/zoo` (an
  `Animal` tree across two packages with a registry in a third).
- **Survey**: remove "inheritance" and "super calls" from `class_features`/`body` in
  `program/survey.rs`; record the union report. Expected on the union
  (`--survey --main All` over the survey's sample programs with the JDK's `java.base` sources as library): the free
  share rises from 7.0% to at least 11.5% (the ordering table's first step, +87 pieces), and
  "inheritance" disappears from the table.

### Milestone 3: interfaces

- **Lowering**: `INTERFACE` declarations (top level and nested static), `implementsClause`
  (program interfaces, and `java.lang.Comparable` as a runtime interface in single-file mode),
  interfaces extending interfaces, abstract/default/static interface methods, interface
  constants (`static final` fields), a Java enum implementing an interface. Rejected still:
  other runtime interfaces ("implementing `java.lang.Runnable` not supported yet"), lambdas,
  anonymous classes, functional-interface conversions ("lambdas"), `private` interface methods
  (accept: they are field-level-free object-level methods; fine), generic interfaces until
  milestone 5 (`Comparable<T>` with a program type argument is accepted here by erasing the
  argument: `compareTo(Version)` is the override; the dispatcher takes `Object` once `Object`
  exists in milestone 4, so in milestone 3 the dispatcher on the `Comparable` enum takes the
  enum itself and arms convert with `to_version()`).
- **AST**: `TypeKind::Interface`, `implements`, `runtime_supertypes`.
- **Analysis**: variants for interface types (union over concretes, mixed representations
  allowed); default methods are object-level on the interface enum; `touch` of an interface
  type touches all its concretes; runtime variants of `Comparable` from conversions of
  `String`/boxes/enums; `validate` rejects interface method calls on runtime variants without
  a dispatch table entry (1.9).
- **Codegen**: as milestone 2 for interface enums; `codegen/dispatch.rs` with the runtime
  variant table (`Comparable.compareTo` for String/boxes/enums); static interface methods as
  free functions; constants as `const`/cells of the interface.
- **Runtime**: `java::string_compare_to(a: &str, b: &str) -> i32` (Java's `String.compareTo`:
  UTF-16 code unit comparison, then length difference) — if the String agent has added it
  under another name by then, use theirs.
- **Tests**: shape tests `an_interface_is_an_enum_over_its_implementors`,
  `default_methods_live_on_the_interface_enum` (`fn describe_in_shape(&self)`),
  `interface_statics_are_free_functions_and_consts`,
  `an_enum_constant_implements_an_interface` (`Shape::Color(Color)`),
  `comparable_dispatches_to_strings` (an arm `Comparable::String(a) => java::string_compare_to(a, ..)`).
  Corpus: `corpus/features/Interfaces.java` (two interfaces, a class implementing both,
  default method overridden in one class and inherited in another, interface constant,
  static interface method, calls through each interface type, upcasts between interface and
  class types), `corpus/features/ComparableImpl.java` (a `Version` implementing `Comparable`,
  sorting an array by hand with `compareTo`, a `Comparable[]` holding strings and versions),
  `corpus/programs/Simulation.java` (an `Actor` interface with default `tick`, `Rabbit`/`Fox`
  classes and a `Season` enum implementing `Actor`, a world array), `corpus/programs/Tokenizer.java`
  (a `Token` interface over `Word`/`Number`/`Symbol` classes, a hand-written scanner).
- **Survey**: remove "interfaces"; expected union free share ≥ 19.7% (+160 pieces per the
  ordering table).

### Milestone 4: `Object`, `instanceof`, casts and patterns

- **Lowering**: `Type::Object` for `java.lang.Object` in every position (today
  `unsupported(node, "Object")`); `INSTANCE_OF` with and without `BINDING_PATTERN` as
  `ExprKind::InstanceOf { value, ty, binding: Option<String> }` (binding variables are
  `BINDING_VARIABLE` symbols: accept them in `local()`); explicit `TYPE_CAST` to class,
  interface and `Object` types (downcasts, upcasts, interface casts); calls resolved to
  `java.lang.Object.{toString, equals, hashCode}` on program types become `MethodCall`s
  resolved to synthetic method entries (see the AST bullet below) instead of `Native::IdentityHash`/the
  rejection; `getClass()` stays rejected ("reflection"); `Object.clone`/`wait`/`notify`
  rejected by name. `new Object()` accepted as `Object::Plain(Rc<()>)` (a lock or sentinel;
  identity only). `Object` arrays (`Object[]`) accepted as `java::Array<Option<Object>>`
  (needed by milestone 5 anyway); arrays *as* Object values (`Object o = new int[2]`) stay
  rejected until milestone 5 ("arrays as Object values").
- **AST**: `Type::Object`, `ExprKind::InstanceOf`, synthetic methods: `Class::methods` gets,
  appended by lowering, one entry per program type for each of `toString`, `equals`, `hashCode`
  that the program calls on a receiver of that type and the type does not declare: `name`
  `toString`/..., `is_synthetic: true` (new field on `Method`), empty body, class = the type;
  `hierarchy.overriders` link a class's own `equals(Object)` to `Object`'s synthetic one so
  dispatch and group alignment work as for any method. The generator emits a synthetic
  `toString` as the default `Display` (1.7), synthetic `equals`/`hashCode` as identity arms.
- **Analysis**: `variants[OBJECT]` from conversions into `Object` (program classes, String,
  boxes, enums, generic instantiations later); `conversions` for downcasts and
  `instanceof` patterns to enum types (`as_x`); `faithful_boxes` extension (2.7); the
  let-else recognition is a codegen matter. `validate`: string identity (1.6).
- **Codegen** (`codegen/casts.rs`): `InstanceOf` forms of 1.5 (matches!, if-let, let chains
  via `null.rs::let_chain` generalized to "narrowing conditions": a null test or an instanceof
  pattern; `narrow_if`/`early_null_exit` generalized to the early-exit instanceof form),
  downcasts `to_x()` with `java::cast_failed`, `Object` enum with `equals`/`hash_code`/
  `class_name`/`Display`, default `Display` for classes without `toString`, `equals`/`hashCode`
  argument conversion to `Object`, `String.equals(Object)`. Remove `check_text` and the
  rejection "printing an object without toString" (keep `check_text` for arrays: printing an
  array still prints a hash of a runtime type; reject as "printing an array" until arrays
  are Object values).
- **Runtime**: `java::object_hash`, `identity_hash` delegating to it, `java::cast_failed`,
  `java::string_hash` (Java's `s[0]*31^(n-1) + ...` over UTF-16 units, wrapping; coordinate
  with the String agent: if they add `hash_code` for strings first, reuse it).
- **Tests**: shape tests `instanceof_is_a_match_on_the_enum`, `a_pattern_binds_the_handle`
  (`if let Shape::Circle(c) = &s {`), `a_negated_instanceof_that_returns_is_let_else`,
  `downcasts_throw_class_cast_exception` (`fn to_circle(&self) -> Rc<RefCell<Circle>>` with
  `java::cast_failed(other.class_name(), "Circle")`), `object_values_are_an_enum`,
  `equals_takes_the_object_enum` (`fn equals(&self, o: &Object) -> bool`),
  `a_class_without_to_string_prints_its_name_and_hash` (`write!(f, "Point@{:x}",
  java::object_hash(self))`), `boxing_into_object` (`Object::Integer(5)`),
  `strings_in_object_are_not_compared_by_identity` (rejection). Corpus:
  `corpus/features/InstanceOf.java` (every form of 1.5, nested patterns on nullable values,
  instanceof against unrelated types, a failing cast at the end that exits 1),
  `corpus/features/ObjectValues.java` (an `Object[]` of strings, boxes, enums and objects;
  `equals`/`hashCode`/`toString` through `Object`; `Object o = 5` and `(Integer) o`),
  `corpus/features/EqualsHashCode.java` (a `Key` class with both, used in a hand-written hash
  table of `Object` keys), `corpus/programs/Interpreter.java` (an AST of `Expr` interface
  with instanceof-based evaluation and pattern matching alongside dispatch).
- **Survey**: remove "instanceof", "reference casts", "patterns" (for binding patterns in
  `instanceof` only; keep counting `RECORD_PATTERN` and patterns in `switch` under
  "patterns"), "Object values". Expected: the union's "Object values" row (318 pieces) and the
  three smaller rows disappear. The ordering table lists generics before Object values, so it
  gives no cumulative figure for this point: re-run the union survey and record the number in
  the commit message (it must be above milestone 3's).

### Milestone 5: generic classes and methods, erased

- **Lowering** (`lower/generics.rs`): a parser for javac's type strings (`Box<T>`,
  `java.util.List<? extends Shape>`, `capture#1 of ? extends Shape`, `T[]`, `<U>(U,U)U` for
  method symbols, `A & B` intersections → reject "intersection types"); `typeParameters` with
  `bounds` on classes and methods into `TypeParam`; `Type::Param(owner.name)` resolved against
  the enclosing method's then class's parameters; `Type::Generic(name, args)` for
  parameterized uses; `erasure(ty)` = the first bound's erasure (`Type::Object` without
  bounds; a class bound's class; an interface bound's interface); declared member types are
  kept as written (`Param`, `Generic`) and use sites get `Cast(substituted, expr)` /
  `Cast(declared, arg)` by the same `convert` helper (section 1.11); `(T) x` and `x instanceof T`
  with a type variable → `Cast(erasure(T), x)` / `InstanceOf` against the erasure (javac's
  meaning); generic arrays `T[]` → `Array(erasure(T))`; raw types; wildcards; diamond
  (`javaType` already shows the inferred arguments); generic constructors; static generic
  methods and generic instance methods; `Comparable<T>` and other generic interfaces as
  supertypes with the bridge conversions in dispatcher arms (an arm converts each argument
  from the group's erased parameter type to the target's declared parameter type with
  `to_x()`, as javac's bridge method does). Rejected still: real generics (none yet; all
  erased), lambdas, `for-each` over anything but arrays (`Iterable`), `Class<T>`,
  `T.class`, varargs of generic types (`T...` erases to `Object[]`, accepted as such).
- **AST**: `Type::{Param, Generic}`, `TypeParam`, `Method::type_params`, `TypeDecl::type_params`.
  `Type::is_reference` includes them; `rust_type`'s unreachable list grows.
- **Analysis**: every `Param` erases in `ty_name`-level decisions (nullness keys, element
  types, `nullable_elements` by erased type), `variants[OBJECT]` grows with everything that
  flows into a type variable's erasure, `conversions` with every `(Object, X)` checkcast the
  use sites need. Arrays as Object values: `Variant::Array(elem)` with variant names
  `IntArray`, `StringArray`, `ObjectArray`; `Object[]` as a field/local type is already
  `Array<Option<Object>>`.
- **Codegen**: `ty_name(Param)` = `ty_name(erasure)`, `ty_name(Generic(c, _))` = the class
  `c` (erased struct); `Cast` codegen for erased members (1.11); dispatcher bridges; array
  variants' arms for `equals`/`hashCode`/`toString` (identity, identity hash, `[I@hash`).
- **Runtime**: nothing new beyond milestone 4 (`Array<T>` already is `Rc<RefCell<Vec<T>>>`,
  so `identity_hash` applies).
- **Tests**: shape tests `type_variables_erase_to_object` (`struct Box {\n    value: Object,`),
  `use_sites_check_cast` (`bi.borrow().get().to_integer()`), `bounded_type_variables_erase_to_the_bound`
  (`fn max(a: Comparable, b: Comparable) -> Comparable`), `a_generic_interface_bridge_converts_the_argument`
  (`Comparable::Version(o) => o.compare_to(&other.to_version())`), `wildcards_and_raw_types_erase`,
  `generic_arrays_are_object_arrays`. Corpus: `corpus/features/GenericsErased.java` (`Box<T>`,
  `Pair<K,V>`, a generic method with a `Comparable` bound over strings, boxes and a program
  class, raw use, wildcard parameter, a generic array `(T[]) new Object[n]`, bounded by a
  program class), `corpus/programs/ArrayListLike.java` (an `Object[]`-backed `MyList<E>` with
  grow, get, set, remove, indexOf via `equals`, used with strings and objects),
  `corpus/programs/HashMapLike.java` (a `MyMap<K,V>` with buckets of `Entry<K,V>` using
  `hashCode`/`equals` through `Object`), `corpus/projects/collections` (the two above split
  into packages with a client).
- **Survey**: remove "generic classes", "generic types", "generic methods". Expected on the
  union: these rows (1142, 982, 116 pieces) disappear; by the ordering table the free share
  should reach at least 60% once inheritance, interfaces and generics are supported, and with
  Object values about 69%. Record the actual figure; the remaining rows should be exceptions,
  inner classes, lambdas, local/anonymous classes, for-each over collections, method
  references, reflection, records.

### Milestone 6: real generics where they fit

- **Analysis** (`analysis/generics.rs`): a generic class or method is *real* when all hold:
  (1) each of its type parameters occurs only in typed positions: field types, parameter and
  return types, local types, type arguments of other real generics, array element types only
  of `Array<T>` created from an existing array (`new T[]` cannot occur; `(T[]) new Object[n]` is
  an unchecked cast and disqualifies); (2) no `Cast` or `InstanceOf` in its body mentions a
  type variable other than the no-op casts lowering inserted for its own members with matching
  instantiation (i.e. the body has no unchecked cast and no `instanceof T`); (3) every
  expression typed by a type variable is only stored, loaded, moved, passed to a parameter of
  the same type variable, returned, compared with `==`/`!=` to `null` or to another value of
  the same variable, converted to text, or the receiver of `equals(same T)`/`hashCode()`/
  `toString()` or of a method of one of its bounds; (4) every bound is `java.lang.Object`, a
  program interface or `java.lang.Comparable` whose methods are all abstract or default
  methods that use `this` only to call the interface's own methods (the trait condition), and
  a bound of the form `I<T>` (the parameter itself) or `I<X>` with `X` concrete; a class bound
  disqualifies; (5) every use site in the program is fully parameterized with concrete,
  wildcard-free type arguments (no raw use, no `?`), each argument a reference type that is
  itself real or non-generic; (6) its supertypes are non-generic or real, and no subtype of it
  is erased; (7) for a generic class, its own `variants` entries appear per instantiation
  (`BoxOfInteger`) wherever it flows into an enum; (8) for a generic method, the inferred
  type arguments at each call are derivable from the arguments' javac types (javac gives the
  substituted parameter types in `javaType` of the arguments; when the method's only use of
  `T` is the return type with no argument mentioning it, Rust cannot infer `T`: disqualify).
  Otherwise the erased form stands (never a rejection). `real_generics`, `traits`, `values`
  recorded; `validate` has nothing to add.
- **Codegen** (`codegen/generics.rs`): `struct Box<T>`, `impl<T: java::Value + Bound> Box<T>`,
  `fn max<T: Comparable<T>>(..)`, trait declarations for `traits` (one method per abstract
  method, default bodies for defaults meeting the trait condition), `impl Trait<Args> for
  <representation>` for every class implementing the interface whose representation can be a
  type argument, and for runtime types from a table (`Comparable<String> for String`,
  `Comparable<i32> for i32`, ...); `impl java::Value for <representation>` for every type in
  `values`; `ty_name(Param)` = `T`, `ty_name(Generic(c, args))` = `C<args>` under the usual
  wrapper; `Cast` of a real `Param` is a no-op; `T`-typed `==` is `.same()`, text `.to_text()`
  (the parts machinery pushes `Part::Arg(Code::atom(format!("{v}.to_text()")))`), `equals`/
  `hashCode` through the trait; variant names per instantiation.
- **Runtime**: `pub trait Value: Clone { fn to_text(&self) -> String; fn hash_code(&self) ->
  i32; fn equals(&self, other: &Self) -> bool; fn same(&self, other: &Self) -> bool; }` with
  impls for `String`, `i32`, `i64`, `f64`, `f32`, `i16`, `i8`, `u16`, `bool`, `Boxed<T>`,
  `Array<T>`, `Option<T: Value>` (null-aware: `to_text` of `None` is `"null"`, `equals` of two
  `None` is true, `same` likewise) in a new `crates/runtime/src/value.rs`.
- **Tests**: shape tests `a_simple_container_is_a_rust_generic` (`struct Box<T> {`, `impl<T:
  java::Value> Box<T> {`, `let v: i32 = bi.borrow().get();`),
  `a_bounded_generic_method_gets_a_trait_bound` (`fn max<T: Comparable<T>>(a: T, b: T) -> T`,
  `trait Comparable<T> {`), `a_class_with_an_unchecked_cast_stays_erased`,
  `a_raw_use_keeps_the_class_erased`, `type_arguments_choose_variants_per_instantiation`
  (`Shape::BoxOfInteger(`). Corpus: `corpus/features/GenericsReal.java` (`Box<T>`, `Pair<K,V>`
  with `toString` over `K`/`V`, `Node<T>`/`Stack<T>`, `max`, instantiated with strings,
  integers, a program class and an interface type; a `Box<Box<Integer>>`),
  `corpus/programs/LinkedQueue.java` (a generic queue and an iterator-free traversal with
  `equals` on elements), `corpus/programs/BinaryHeap.java` (`Heap<T extends Comparable<T>>`
  over integers and a `Task` class).
- **Survey**: no row changes (generics are already uncounted); record how many generic
  classes of the union became real (`--survey` can print "generic classes translated as Rust
  generics: N of M" from the facts: add that line in this milestone).

### Order and dependencies

1 → 2 → 3 → 4 → 5 → 6. Milestone 4 could precede 3 (it needs only 2) if the implementer
prefers; 5 needs 3 and 4 (Comparable bridges, Object). Each milestone ends with the README's
"Current state" paragraph updated, the corpus green, and the union survey re-run with its
numbers written into the commit message.

## 4. Risks and fallbacks

- **Superclass constructors that leak `this` or call overridable methods**: their class tree gets
  the second constructor form (1.4), which runs each constructor body on the whole object; such
  a class with an enclosing instance or captured locals is still rejected. JDK: the stream
  pipeline's `AbstractPipeline` constructor links the previous stage to `this` and calls the
  overridable `opIsStateful()`, so the stream classes use that form.
- **Field-level versus object-level misclassification**: an object-level body is always
  correct (it has the whole object); field-level is an optimization. If in doubt classify as
  object-level. A field-level body must never call a method that has overriders: the
  classification checks `hierarchy.overriders` transitively over the whole group.
- **`Ref` from `base()` held across a mutable borrow** (`b.base().x + b.set(..)` in one
  statement): the existing `conflicts`/`guarded` machinery scopes field reads of non-plain
  objects in braces; enum-typed objects count as non-plain in `is_plain` (keep them out of
  `is_plain` unless `ThisForm::Enum`'s `self` in an immutable family). A dispatcher call
  counts as `(1, writes)` in `borrows_here`. If a RefCell panic shows up in the corpus, the
  fallback is the bound-call form (`bound_call`) which already binds receiver and arguments
  first.
- **Variants that no value reaches**: variants(T) is every non-owned concrete program class
  under T plus the runtime types that flow in (1.1). An owned class is absent (owned implies
  never upcast), so `instanceof`/casts naming it become constant `false`/failing; the shape
  test `a_class_that_is_never_upcast_stays_a_plain_value_beside_its_siblings` pins it. A
  non-owned class that happens never to reach T still has a variant: harmless dead arms,
  and no analysis beyond `owned` is needed to build the enum.
- **Families and immutability**: unifying immutability over a class tree loses `Rc<C>` for
  a leaf whose sibling is mutable; acceptable, documented in the README.
- **`toString`/`equals`/`hashCode` defaults print or hash addresses**: the corpus must not
  print them (as with Java's own random hashes); a translation that depends on them is still
  "faithful enough". `Object.hashCode` semantics (stable per object) hold through `Rc`.
- **Identity of strings and plain boxes inside enums**: rejected (strings) or made faithful
  (boxes). Never compare by value where Java compares references.
- **Generic erasure corner cases**: `capture#N` types, intersections (`T extends A & B`,
  rejected: "intersection types"), recursive bounds beyond `T extends Comparable<T>` (erased:
  always fine), static members of generic classes (not parameterized: fine), inner classes
  of generic classes (rejected with "inner classes" as today), `Class<T>` (rejected,
  "reflection"), `T...` varargs (erased to `Object[]`: fine).
- **Real generics are a refinement**: when any condition of milestone 6 is unclear, the class stays
  erased; the erased path must be complete before milestone 6 begins. Rust inference failures
  on a real generic method (condition 8) are prevented, not patched with turbofish.
- **Runtime interfaces**: only `Comparable` in this plan; `Iterable`/`Iterator` wait for
  for-each over collections, `Runnable`/functional interfaces for lambdas. A class
  implementing them is rejected, not partially translated.
- **Name clashes**: a dispatcher `m` and an inherited `m_in_d` on one enum; overloads get
  type suffixes per `impl` as today (`method_names` must allocate per emitted `impl`:
  fields struct, concrete class, enum). A program class named `Object`, `String`, `Integer`,
  ... is rejected. A field named `base` in a class with an embedded `base` is renamed
  `base_` by `snake_case`-style uniqueness (add `base` to the reserved names when the struct
  embeds one).
- **Must stay rejected for now** (with their current messages unless noted): inner (non-static)
  classes, local and anonymous classes, lambdas and method references, exceptions (`try`,
  `throw`, `extends Exception`: "extending `java.lang.Exception` not supported yet"), records,
  `for-each` over collections, `switch` patterns, `getClass()`, `clone()`, intersection types,
  implementing runtime interfaces other than `Comparable`, extending runtime classes other
  than `Object`, calling runtime methods on `Object` values beyond `toString`/`equals`/
  `hashCode`, `synchronized` on arbitrary objects (today's `NullCheck` form remains), and
  `Object` values holding arrays until milestone 5.

## 5. Deviations from the plan

Decisions that changed during the implementation, milestone by milestone.

### Milestone 1

- `super.m(args)` is not a new `SuperCall` kind: it is a `MethodCall` with `is_super: true` whose
  receiver is a synthesized `this` of the superclass's type, so every analysis that already
  handles calls on `this` sees it without change. Fields get no `owner` in the AST either: the
  declaring class follows from the receiver's static type and the hierarchy
  (`Hierarchy::field_owner`), and `Facts::field_owners` maps (class, field) to it for the keys
  of `nullable_fields`.
- A class with subclasses gets a fields struct not only when it declares instance fields, but
  also when it has objects of its own, when it embeds an ancestor's fields struct, or when one of
  its constructors does something. A superclass without fields whose constructor prints is then
  an empty `BaseFields {}` that its constructor builds, which needs no special `init_new`
  function; a superclass without a fields struct has a constructor that does nothing, and only
  the effects of the arguments of `super(...)` are written, as statements.
- A field-level method may also call field-level methods through `super`, and it must not reach
  an object of its class tree through another handle while it runs (unless the tree is
  immutable): it borrows the fields struct for the whole call, like a method of today's classes
  that takes `&self`. Methods that are not field-level are emitted in the concrete class's
  `impl`, under their own name; `_in_<class>` is appended only when the concrete class (or a
  class between) overrides them, so that `super` calls reach them.
- Instance methods are named once per override group within a class tree, so that every
  implementation has the name its callers use; overload suffixes count the groups of a name in
  the tree.
- An owned class (a plain value) is never one with subclasses, and never one whose objects run an
  inherited method that is not field-level, since such a method takes the object as a value.
- `analysis::analyze` returns a `Result`: the rejection of a superclass constructor that is not
  a plain-value builder needs the borrowing analysis.
- An inherited field-level `toString` becomes the `Display` of the superclass's fields struct;
  the concrete class's `Display` delegates to it (`fmt::Display::fmt(&self.base, f)`).
- A field named `base` in a subclass is rejected ("a field named `base` in a subclass"): it
  would clash with the embedded fields struct.
- `mutating` and `writers` are spread over override groups and `isolated` holds only for whole
  groups, which keeps the `&mut self` choice and the borrow-conflict counts right for a call that
  may run any member of a group.
- `corpus/features/SuperInit.java` is `corpus/features/SuperclassConstructors.java`, and the
  `super` cases are in `corpus/features/SuperCalls.java`. `corpus/programs/Ledger.java` has two
  class trees (accounts and reports, the report a template method); it avoids `StringBuilder` and
  exceptions, which other work adds.

### Milestone 2

- No `variants`, `single`, `dispatched` or `conversions` facts: `hierarchy::rep` decides per type
  whether its values are one class's object or an enum, and the generator finds which enum items
  the code uses (`base()`, `base_mut()`, each dispatcher, each body emitted on an enum, each
  `to_x()` conversion, `Display`) while it writes the code (`codegen/dispatch.rs`, `Need`). It
  writes the program again with those items until no new one is asked for, which takes two or
  three passes. An enum gets only the dispatchers that some call uses, so it has no dead code.
- The variants of an enum are the classes under the type that are not owned and have a struct:
  instantiated, or the type of something. A variant is named after its class's simple name
  (`Shape::Circle`), with the outer classes' names when two variants of one enum share it.
- A dispatcher arm whose class runs a body emitted on the enum of a type *below* the enum's type
  wraps the object in that type's enum (`Binary::Add(o.clone()).eval()`) instead of converting the
  whole value, which would be a downcast for the other variants.
- When every variant runs the body emitted on the enum itself, a call names that body and no
  dispatcher is generated. A `toString` emitted on an enum is always `to_string_in_<type>`, since
  `to_string` is the enum's `Display`, and `x.toString()` on an enum is `x.to_string()`.
- `==` on enum values is `&a == &b`, and with a value that may be null `a.as_ref() == Some(&b)`;
  both through the identity `PartialEq`. `base_mut()` exists only for a mutable family.
- An override that returns a value of another representation than the method it overrides (a
  covariant return of an enum type's subclass) is rejected: "covariant return types not supported
  yet". Dispatchers forward results unchanged.
- Lowering converts the elements of array initializers and of packed varargs to the element type;
  milestone 1 converted only the other places of section 0's list.
- The nullness analysis now also looks at field initializers, where `new T[n]` leaves the elements
  null (this fixed `known-failures/StringArrayFieldInitializer.java`).
- A variable mentioned twice in one statement is not moved there, since Rust keeps the temporaries
  of the statement (a `Ref` of a field read, the references `format_args!` takes) alive to its
  end; a mention in the condition of `?:` does not count, as an `if` drops its condition's
  temporaries before a branch runs. A `for` loop's update is guarded like a statement, so
  `at = at.next` scopes its borrow there too.
- Survey: "inheritance" is gone; a class that extends a class outside the sources is reported as
  "extending classes outside the sources", and "super calls" is no longer a feature.
- Corpus: `corpus/features/{Dispatch,Identity,Families,ObjectLevelInheritance,
  InheritedFieldUpdates}.java`, `corpus/programs/{Shapes,Expressions}.java` and
  `corpus/projects/zoo` (packages `zoo.animals`, `zoo.keep`, `zoo.app`).

### Milestone 3

- The variants of a type's enum are no longer every concrete class under it that is not owned,
  but those whose objects become values of the type (`hierarchy::flows`, a fact): a `new`, an
  upcast (`Cast`) from a type whose values they are, a call of a method the type declares on a
  receiver of a subtype, `super.m()`, and running a method of the type that is not field-level
  (its `this` is a value of the type). This is the plan's "variants(T)" fixpoint; milestone 2 had
  left it out, and interfaces made it matter: a class implementing an interface is usually the
  superclass of classes that are only ever used through the interface, and without it its type
  became an enum with variants that nothing constructs (rustc's dead-code warning). Such a class
  is now a plain struct again (`Rect` beside `Square`).
- `Comparable` is not a runtime interface with String, box and enum variants in this milestone:
  a class may implement `Comparable<T>` and calls `compareTo` directly, but `Comparable` as a type
  (`Comparable[]`, a parameter) is rejected (`type java.lang.Comparable<..>`), as are the other
  JDK interfaces ("implementing `java.lang.Runnable` not supported yet"). Runtime variants come
  with `Object` in milestone 4, which needs the same machinery; `comparable_dispatches_to_strings`
  and the `Comparable[]` half of `ComparableImpl.java` move there.
- An interface with exactly one implementing *class* is that class's object, as in milestone 1.
  An interface whose only implementor is a Java enum is still an enum with one variant
  (`Coded::Color(Color)`): everywhere else a value of a class type is an object (`Option<Rc<..>>`
  for null, identity through `Rc::ptr_eq`), and the one-variant enum keeps that true.
- Java enums are concrete types of the hierarchy (`Hierarchy::concretes` includes them) and never
  owned-filtered; the variant holds the `Copy` constant, compares with `==`, displays through the
  enum's `Display`, converts with `*o` instead of `o.clone()`, and calls its methods as `o.m()`
  (they take `self`). A default method of an interface whose only variant is a class lives in that
  class's `impl`; for an enum, in the enum's `impl` with `self` by value.
- An interface method that an enum implements with a method every enum has (`name()`,
  `ordinal()`, `toString()`) dispatches to `o.enum_name()`, `*o as i32` or the enum's `Display`;
  any other method of `java.lang.Enum` doing so is rejected in `validate`.
- `X.super.m()` in a class is a `super` call of the default method `m` of interface `X`.
- javac does not mark an interface's fields `static` or its methods `abstract` in the tree, so
  `STATIC` comes from the declaration's symbol (lowering and reachability) and a method of an
  interface without a body is abstract.
- A parameter that a method's body never mentions is named `_x`, so a default that overrides
  replace (`default boolean isSymbol(char c) { return false; }`) has no unused-variable warning.
- Whole programs now get what single files had: each class's `<clinit>`, enum constants (and
  constant bodies), and the enum items in their package's module (`lower::finish_types`); before,
  an enum in a project panicked in the generator.
- Survey: "interfaces" is gone; implementing a generic interface is "generic classes", and
  implementing an interface that is not in the sources is "implementing interfaces outside the
  sources". Free share: 12.1% to 20.3%.
- Corpus: `corpus/features/{Interfaces,InterfaceDefaults,EnumsImplementingInterfaces,
  ComparableImpl}.java`, `corpus/programs/{Simulation,Tokenizer}.java` and
  `corpus/projects/library` (packages `library.items`, `library.desk`, `library.app`, with an
  interface, its constant and static method, a `Comparable` class and an enum implementing it).

### Milestone 4

`instanceof`, casts and patterns between the program's own types came first, before `Object`:

- A pattern variable is an owned value, not a reference into the tested value: the enum of the
  tested type gets `fn as_circle(&self) -> Option<Rc<Circle>>` (`Need::Narrow`), and
  `x instanceof Circle c` binds `c` from `x.as_circle()` (`Option<Rc<..>>` cloned out of the
  variant). The plan's `if let Shape::Circle(c) = &s` would make `c` a reference that the rest of
  the generator does not know how to read (a narrowed object is a `&Rc<..>` only for null tests
  today), and a binding to a type with an enum of its own (`o instanceof Base b`) needs the
  converting form anyway; one form covers both. The cost is an `Rc` clone per successful test.
- Where the variable is bound: an operand of an `&&` chain (a let chain in an `if`, an `else if`,
  a `while` or a `?:`; `is_some_and(|c| ..)` in any other expression), `if (!(x instanceof C c))
  .. else ..` (`if let` with the branches swapped) and `if (!(x instanceof C c)) <leave>;`
  (`let .. else`). `analysis::patterns` rejects a read of a pattern variable anywhere else (for
  example `!(x instanceof C c) || c.v == 0`, a `for` condition, or after a `while (!(..))`) with
  "a pattern variable used here not supported yet". The plan's recognition of
  `if (!(o instanceof Rect)) return; Rect r = (Rect) o;` as a `let .. else` is not done: the
  cast is a checked `as_rect()` of its own, which is correct and only one `match` slower.
- Casts: `(Circle) s` is `java::cast(s.as_circle(), "Circle")`, a runtime function that returns
  the value or throws `java.lang.ClassCastException` (`java::cast_failed`), instead of a
  generated `to_circle()` with a failing arm. The message names only the target class (`cannot
  be cast to class Shapes$Circle`): naming the object's class needs a `class_name()` per enum,
  which waits for `getClass()`. A cast whose operand's class is known and is not below the
  target always fails, and is written as that failure. A cast or test to a supertype is the
  implicit upcast that already existed.
- A cast of a value that may be null to a Java enum (`(Suit) item` where `item` may be null) is
  rejected ("casting a value that may be null to an enum not supported yet"): an enum is an
  `Option` only in a local assigned `null` (`lower::enum_nulls`), and the cast would make one
  anywhere.
- `flows` has an edge for a pattern's binding, as for a cast: a class whose objects are bound by
  a pattern to a type becomes a variant of that type's enum.
- `else if` conditions are let chains too (they were plain conditions), so
  `if (s instanceof Circle c) .. else if (s instanceof Rect r) ..` binds in every branch; an
  `else if` that `narrow_if` writes (a negated pattern with an else branch, or `s == null` with
  one) is nested instead.
- Survey: an `instanceof` between declared types (with a binding pattern) and a cast between
  declared types are no longer features; the rest is "instanceof on types outside the sources"
  and "reference casts". Record patterns and patterns in `switch` stay "patterns". The free share
  stays 20.3%: in the JDK the tests and casts are on `Object` and generic values.
- Corpus: `corpus/features/InstanceOf.java` (every form above, nullable operands, `this`, a
  test that is statically false, and a failing cast that exits 1), `corpus/programs/
  Interpreter.java` (a parser, evaluator, simplifier and printer over a node tree, by patterns),
  and `Loans.bookPages`/`discs`/`first` in `corpus/projects/library` (patterns on array
  elements that may be null, a cast across packages).

`Object` came second:

- `Object` is not a `Type::Object` but a synthetic interface type of the program, `OBJECT`
  (`lower/object.rs`), which every type without a superclass implements, so the machinery of
  interfaces (the enum over the classes whose values flow into it, conversions, dispatchers,
  `as_x`, `instanceof`) covers it unchanged. Strings and the eight boxes are types of kind
  `TypeKind::Runtime` beside it, whose variants hold the plain value (`Object::String(String)`,
  `Object::Integer(i32)`); a primitive converted to `Object` becomes its box's variant. Lowering
  adds these types only to a program that mentions `java.lang.Object` (other than the implicit
  `super()`), so other programs translate exactly as before.
- `equals(Object)` and `hashCode()` are two synthetic abstract methods of `Object`, appended after
  the program's methods, which the program's own `equals(Object)` and `hashCode()` override (the
  plan had one synthetic method per type and call). A variant whose class overrides neither gets
  an arm in the dispatcher: identity (`Rc::ptr_eq`, `java::address_hash`) for an object, value
  equality and Java's hash for a string or a box (`Double`/`Float` compare `to_bits`, as
  `Double.equals` does), and for an enum's constant `==` and its ordinal (Java's hash is an
  identity hash, which no program can depend on). The same call on a single class without an enum
  is the identity directly (`identity_call`). `String.equals(Object)` and `Integer.equals(Object)`
  with an argument of `Object` are `Object`'s `equals` on the receiver converted to `Object`.
  `Object.toString()` is `Builtin::ObjectText`, the enum's `Display`.
- The identity hash is the address of the class's struct (`java::address_hash` of
  `RefCell::as_ptr`, `Rc::as_ptr` or `ptr::from_ref`), not of the `Rc`, so that a method that
  borrows the object (`self: &Point`) hashes the same object as code that holds its handle;
  `System.identityHashCode` and the inherited `hashCode()` that existed before use it too (the
  plan's `java::object_hash`).
- A class without `toString` prints as `java::default_text("Point", hash)` (its binary name, `@`
  and the hash in hexadecimal), with the class's own `hashCode` when it has one, in an enum's
  `Display` and for a plain class alike. `check_text` and the rejection "printing an object
  without toString" are gone, which fixes the known failure `NullObjectConcat.java`; printing an
  array was never routed through `check_text` and is unchanged.
- `new Object()` stays rejected (the plan accepted it as a sentinel): it needs a variant of its own
  and is mostly a lock, which needs `synchronized`. `class_name()` waits for `getClass()`, which
  stays rejected ("reflection"); so does an array as an `Object` ("an array as an `Object` value",
  until milestone 5).
- `==` between two `Object`s is rejected only when both sides may be strings or boxes (`Object`'s
  variants include a runtime type and neither side is a conversion from the program's own type),
  because a string or a box in an `Object` is a value whose identity is not kept. The shortcut
  `a == b || a.equals(b)` (`Objects.equals`) is accepted: its result is `equals`'s either way.
  Later, an operand may also assign its variable on the way, as `HashMap.getNode` writes
  `(k = first.key) == key || (key != null && key.equals(k))`; the assignment's value is then a
  clone of the variable's handle, which the `equals` reads again.
- `TreeMap`'s sub-map iterators stop at the end of the view with `next.key != fenceKey`, where
  `fenceKey` is `fence == null ? UNBOUNDED : fence.key`. In `TreeMap`'s own code, a comparison of
  two keys of its entries (or of a field that only ever holds one, `null` or a sentinel) is
  accepted and compares by value. That gives Java's result: the keys of a sorted map are distinct
  under its comparator, and in the translation a comparator sees strings and boxes only by
  value, so two keys that are equal values compare as 0 and cannot both be in the map. The rule
  is that narrow because the argument rests on that invariant: in general, two equal strings
  held by different nodes are different objects in Java.
- Two analyses had to learn about the new sources of values: a string bound by a pattern
  (`o instanceof String s`) is not known to be ASCII, and the generator's nullness of a cast
  follows its operand, which a null check may have narrowed.
- Survey: an `Object` value is no longer a feature, nor a test or cast from `Object` to the
  program's types, strings and boxes; `getClass()` is "reflection" and `Object`'s other methods
  (`clone`, `wait`, `notify`) are "methods of Object". The free share goes from 20.3% to 22.5%
  (library 377 to 420 pieces).
- Corpus: `corpus/features/ObjectValues.java` (strings, boxes, enums and objects as `Object`s,
  typed tests and casts, a field and a parameter of `Object`, identity of a class without
  `equals`, a failing cast that exits 1), `corpus/features/EqualsHashCode.java` (`equals` and
  `hashCode` through interfaces and an abstract class, `Object[]` of mixed values, the default
  `toString`), `corpus/features/NullObjectConcat.java` (moved from the known failures),
  `corpus/programs/PropertyBag.java` (a settings store of `Object` values with typed getters),
  and `equals`, `hashCode`, `contains(Object)` and an `Object` label in `corpus/projects/zoo`.

### Milestone 5

Generics are erased in lowering, and nothing after it knows they existed:

- There is no `Type::Param` or `Type::Generic`. `lower/generics.rs` reads javac's type strings: a
  type variable is the erasure of its first bound (`Object` without one, `Comparable` for
  `T extends Comparable<T>`, a class for `S extends Shape`), looked up in the method being lowered
  and then its class and the classes it is nested in; a parameterized type (`Box<Integer>`,
  `Entry<K, V>[]`) is its class; `? extends X` and `capture#1 of ? extends X` are `X`, and `?` and
  `? super X` are `Object`. A type variable hides a class of the same name (a class `T` beside
  `Box<T>`). Members are declared with the erased types; a use whose type javac substituted (a
  call's result, a field, an element) gets `Cast(substituted, use)` from the declared type, which
  is javac's checkcast and the generator's `java::cast(x.as_integer(), ..)`. Arguments convert to
  the erased parameter types through the conversions every call already makes. Milestone 6 has to
  add the types it needs to keep a type variable instead.
- `Comparable` is not the runtime-interface table of 1.9 but a second synthetic interface,
  `COMPARABLE`, beside `Object`, planned only when the program uses `java.lang.Comparable` as a type
  outside an `implements` clause. Strings, the boxes and the program's enums implement it, and so
  does a class that says `implements Comparable<..>`. Its abstract `compareTo(Comparable)` has an
  arm per runtime type and enum in `codegen::dispatch::compare_arm` (`java::compare_to`, `cmp`,
  `java::double_compare`, `java::bool_compare`, a difference of `char`s or ordinals), which throws
  `ClassCastException` when the argument is of another class. A program that only calls a class's
  `compareTo` directly translates as before.
- `java.lang.Number` is a third synthetic interface of the same kind, `NUMBER`, planned when the
  program uses it as the type of a value (a variable, a bound such as `T extends Number`, the
  elements of a `List<? extends Number>`). The six numeric boxes implement it, and its abstract
  `byteValue()` to `doubleValue()` have an arm per box in `codegen::dispatch::number_arm`: the
  value converted with `as`, which converts as Java's primitive conversions do (a floating-point
  value becomes a `byte` or a `short` through an `int`). A program class that extends `Number` is
  still rejected, as is any extension of a runtime class.
- `java.lang.CharSequence` is a fourth, `CHAR_SEQUENCE`, planned when the program uses it as the
  type of a value (the JDK's `StringJoiner` and `Collectors.joining` take `CharSequence`s). The
  JDK's interface is not translated (it is among `RUNTIME_CLASSES`). Strings implement it, and so
  do builders, through the runtime class that holds a `StringBuilder` as an `Object`, which keeps
  the builder's identity: what is appended later shows through the `CharSequence`. Its abstract
  `length()` and `charAt(int)` have an arm per runtime class in
  `codegen::dispatch::char_sequence_arm`; `toString()` is the value's text and `isEmpty()` a zero
  `length()`. Its other methods (`chars()`, `subSequence`, `compare`) and a program class that
  implements it are rejected.
- Bridges are methods, not conversions in dispatcher arms: when a method overrides one whose erased
  parameter or return types differ (`Integer next()` implementing `T next()` of `Source<Integer>`,
  `compareTo(Point)` implementing `Comparable`'s), lowering adds a method to its class with the
  overridden signature that converts the arguments, calls it and converts the result, and that
  method takes its place in the override group (`object::bridges`). A subclass whose superclass
  already has the bridge gets none. The method keeps its own types and a name told apart by its
  parameter or return type (`next_integer`, `compare_to_point`); the group names are now unique in
  every class tree the group reaches (`codegen::names`). This also lifts "covariant return types"
  for these overrides.
- A varargs parameter of a type variable (`T... items`) is an array of its erasure, read where the
  method declares it, not at the call; an array of references passed for it is passed on, as
  javac infers `T` from it (and is then rejected below unless it is an `Object[]`).
- Arrays do not convert: `String[]` is `java::Array<String>` and `Object[]` is
  `java::Array<Option<Object>>`. A generic array built as `(T[]) new Object[n]` is an `Object[]`
  and translates. Java's arrays of references are covariant, though (a `Version[]` is an
  `Object[]`), and erasure makes the array of a type variable an `Object[]`, so
  `list.toArray(new Version[0])` passes a `Version[]` where `T[]` is an `Object[]`, and
  `toArray(a)` with an array big enough fills `a` and returns it. Copying at the boundary would
  lose that identity. Instead, two different array types of references that meet (an argument,
  an assignment, a return or a cast between the two, a use where javac substituted one for the
  array of a type variable, which is an array of its bound, or an array passed for `Object...`)
  are each an `Object[]` everywhere in the program: lowering notes each such type, and
  `real_generics::retry` lowers the program again with it as `Object[]` (`lower::covariant`). So
  `Integer[]` passed to `<T extends Comparable<T>> int countGreater(T[] array, T pivot)` makes
  both `Integer[]` and `Comparable[]` `Object[]`s. Element reads get the checked cast back that
  erasure writes, a for-each over such an array casts each element to the variable's type, and a
  store converts to `Object`. A Rust generic that would take such an `Object[]` for an array of
  its type variable is erased. Every other array keeps its exact type. Keeping the element class
  in a tag beside the elements would need an array type of its own through all of the array code,
  so it is not done, with two deviations: storing an object of another class in such an array
  does not throw `ArrayStoreException`, and its `getClass()` is `Object[]`'s
  (`[Ljava.lang.Object;`). Arrays of strings are left out: the runtime's string methods make and
  take `java::Array<String>` (`split`, `String.join`, `main`'s arguments), so a `String[]` passed
  for an array of a type variable is still rejected ("an array passed as an array of a type
  variable").
- An array as an `Object` value (`Object o = new int[2]`, the elements of a `Deque<int[]>`) is the
  plan's `Variant::Array`, done as a runtime class beside the strings and the boxes rather than a
  variant kind of its own: each array type that is converted to `Object` or cast back from it gets
  a `TypeKind::Runtime` declaration named after the type (`int[]`, `String[][]`), collected while
  lowering (`lower::object::note_object_array`) and added after `Object`'s other runtime types.
  Its variant holds the shared `java::Array` (`Object::IntArray(java::Array<i32>)`, with
  `as_int_array` and `to_int_array`), so the array keeps its identity: `==` and `equals` compare
  with `Rc::ptr_eq`, `hashCode` is `java::identity_hash`, and it prints as Java's default
  `toString`, its JVM name and hash (`[I@1b6d3586`). Arrays of a Rust generic's instantiation
  (`Box<String>[]` under real generics) have no runtime class and are rejected.
  A `StringBuilder` as an `Object` (a `Supplier<StringBuilder>`'s value, the elements of a
  `List<StringBuilder>`) is done the same way: the runtime class `java.lang.StringBuilder` is added
  when a builder is converted to `Object` or cast back, its variant holds the shared
  `java::StringBuilder`, `==`, `equals` and `hashCode` are the builder's identity (`same`,
  `identity_hash`), and it prints as its text (`ast::hierarchy::shared_of` gives the type of
  either kind of runtime class). In a program with threads it is rejected, since the builder is
  an `Rc` that an `Object` crossing threads cannot hold.
  `System.arraycopy` where the source or the destination is such an `Object` (the JDK's code
  generic over the kind of array, `SpinedBuffer.OfPrimitive`'s `T_ARR`) converts both to `Object`
  and matches their variants: two arrays of one type are copied as any two arrays are, anything
  else throws `ArrayStoreException` (`codegen::dispatch::object_array_copy`). Deviations: the
  message is always "arraycopy: type mismatch", and a copy between two different reference array
  types throws even where every element would fit (an `Object[]` of `Integer`s into an
  `Integer[]`); arrays of references that meet are one `Object[]` type anyway.
- A value read back out of a generic container is a cast from `Object` that may be null (the
  container's array has null elements), so reading an enum back (`Suit s = list.get(0)`) is
  rejected by the existing "casting a value that may be null to an enum"; strings, boxes and
  classes read back fine. The enum-field check was too strict for the classes that hold enums: an
  enum field without an initializer may now be set by any of the leading field assignments of
  every constructor, not only the first statement (`lower::enums::set_before_use`).
- Intersection bounds (`T extends A & B`) were first rejected ("intersection types"). A type
  variable with several bounds now erases to its first bound, and the receiver of a call of a
  method that a later bound declares is cast to that bound (`lower::generics::receiver`), which is
  the checkcast javac's erasure writes there; an argument or a store converts to a later bound by
  the conversions every use already makes. `Collections.max` and `min`
  (`T extends Object & Comparable<? super T>`) translate this way. Generic
  constructors, explicit type arguments (`G.<Long>boxed(5L)`), raw types and diamonds need nothing
  of their own.
- Survey: "generic classes" and "generic methods" are no longer features, "generic types" counts
  only parameterized types from outside the sources (`java.lang.Comparable` is inside), and a cast
  to a type variable, to a generic array or between the program's generic types is not a
  "reference cast". The free share goes from 22.5% to 62.2% (library 420 to 1184 pieces, program
  16 to 20).
- Corpus: `corpus/features/GenericsErased.java` (bounded type variables, wildcards, a subclass of
  a parameterized class, `(T[]) new Object[n]`, a generic method of a generic class, nested
  `Box<Box<String>>`, explicit type arguments), `corpus/features/GenericBridges.java` (generic
  interfaces implemented with type arguments, `Comparable` bounds over boxes, strings, an enum and
  a class), `corpus/programs/ArrayListLike.java`, `corpus/programs/HashMapLike.java`, and
  `corpus/projects/collections` (a generic interface with a default method, two implementations,
  a `Pair<A, B>` with `equals` and `hashCode`, and algorithms bounded by `Comparable`).

### Milestone 6

Stage one of the plan: the generics that need no trait. Text, `equals`, `hashCode`, bounds and
per-instantiation variants are not done, and a generic that needs them stays erased.

- There is no `Type::Param`. A type variable of a real generic is a synthetic `TypeDecl` of kind
  `TypeKind::Param`, named after its owner (`Box#T`, `@12#T` for a method), with no members and no
  implementors: every analysis already treats a value of a class it knows nothing about as an
  opaque value that is cloned, never borrowed into. `Type::Class` carries its type arguments
  (`Type::Class("Box", [String])`), and `Type::generic` interns them. The generator writes the
  synthetic class as its variable (`T`) and a class with arguments as `Box<String>` inside its
  representation (`Rc<RefCell<Box<String>>>`).
- Realness is decided in lowering, not by a separate analysis over the erased program, since the
  erased program has lost what the decision needs. The candidates are classes with type
  parameters that have no bounds and each occur in the type of an instance field (a Rust struct
  must use its type parameters: `Ring<E>` holding an `Object[]` stays erased), no supertypes, no
  subclasses and are not abstract, and methods
  whose type parameters have no bounds. Lowering translates them as real; afterwards
  `real_generics::verify` checks the lowered program and fails every candidate whose type
  variables' values are used for anything but a store, a return, an argument to a parameter of a
  type variable or a comparison with `null`, whose types meet a cast or an `instanceof`, that is
  used raw or as an array element (`T[]`, `Box<T>[]`), or (for a method) whose type parameters
  are not all mentioned by its parameters' types, which Rust infers them from. Lowering then runs
  again with the failed candidates erased, until nothing fails; a lowering error with candidates
  left is retried with all of them erased, so a real generic never turns a translated program into
  a rejected one. Later, the array of a type variable passed for an erased array (`Arrays.asList`'s
  `T[]` to its list's `E[]`, an `Object[]`) fails only the generics it mentions instead of being
  such an error, so the program's other real generics stay real.
- The type parameters are bounded by `Clone` only (`impl<T: Clone> Box<T>`, `fn pick<T: Clone>`),
  and every use of a type variable's value clones it, as for any value of a class whose
  representation is an `Rc`. A literal passed for a type variable gets its type suffix (`3i32`),
  which Rust could not infer otherwise.
- Use sites keep javac's substituted types: `words.pop().length()` is `String`'s and
  `numbers.pop() + 1` an `Integer`'s, with no cast. A generic class whose constructor builds
  another instantiation (`Pair<B, A> swap()`) needs nothing of its own.
- `use_lines` found `Cell<` in the name of a generic struct (`BoxCell<T>`) and imported
  `std::cell::Cell`; it now matches only the whole name.
- A field of `this` assigned from a borrow of itself (`head = head.next` with a mutable node) kept
  the node's `Ref` alive into the write; the right operand is now a block of its own, as it already
  was for a local (`codegen::objects::reassigns_through_borrow`).
- Corpus: `corpus/features/GenericsReal.java` (a linked stack, a pair whose swap changes its type
  arguments, a cell that may hold null, nested instantiations, generic methods) and
  `corpus/programs/LinkedQueue.java` (a generic queue of generic pairs driving a breadth-first
  search, and a generic tally). The collections project's `Node<T>` became real.

### `Object.clone` in class trees

- `super.clone()` of `Object` is a copy of the object's runtime class, as Java's is, in any class
  of a tree: on the enum of the class (the method that calls it uses `this` as a value, so it is
  object-level) it is a `match` with a struct literal per variant, which reads each field of the
  variant's struct and of the fields structs it embeds as `base`, cloning a field where its type's
  values are cloned (handles are shared, as Java shares the references). On a struct it reads the
  fields through `this` as before. `(C) super.clone()` in `C` is the copy itself, of `C`'s type; a
  cast to another type converts it.
- Whether a class may be copied is `TypeDecl::cloneable`: it, a superclass or an interface it
  implements extends `Cloneable`, which lowering reads from javac's tree since `Cloneable` itself
  is not a type of the program. A variant that is not `Cloneable` throws
  `CloneNotSupportedException` with its binary name (`java::clone_refused`): a failure of the
  runtime (`lower/exceptions.rs` `RAISED`, for `Object#clone`), which a `catch` sees through
  `java::try_clone_refused`, so `EnumSet.clone`'s and `ArrayList.clone`'s `try` translate.
- `AssertionError(Object)` joined `java/Standard.java` (`EnumSet.clone`'s `catch` throws one),
  and `Arrays.copyOf` fails only with `NegativeArraySizeException`, so a `catch` of another
  class around it is no longer rejected.
- Corpus: `corpus/features/CloneInClassTrees.java` (a `Cloneable` abstract superclass copying
  objects of its subclasses, a subclass's `clone` over its superclass's, a superclass that is not
  `Cloneable` clearing a cache in its copy, a subclass whose objects refuse, caught) and
  `corpus/features/CloneRefusedUncaught.java` (the refusal ending the program).
