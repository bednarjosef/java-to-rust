# Natural Rust output (planned)

Today the translator's output is correct but reads like compiler output. This plan is about making
it read like Rust that a person would write: natural names, a sensible module layout, plain values
instead of shared cells, `Vec` and `for` loops instead of Java's collection machinery. The second
half describes a later, separate "port mode" for people who want to take the translated code over
and maintain it by hand.

The deeper analyses this relies on (points-to, shape, effects, what the program observes) are
designed in [thorough-mode.md](thorough-mode.md). This document is the roadmap from the output's
point of view: what looks wrong today, in what order to fix it, and how to measure progress.

## Where the output stands

ShapesAbstract is 167 lines of Java and translates to 2,939 lines of Rust, most of them the JDK
classes it reaches. One loop from it:

```rust
let it = shapes.iterator().unwrap_or_else(java::uncaught);
while it.has_next() {
    let s: Option<ShapesAbstractShape> = it.next().unwrap_or_else(java::uncaught)
        .as_ref().map(|o| java::cast(Object::as_shape(o), "ShapesAbstract$Shape"));
    total += java::non_null(s.as_ref()).area();
```

What a Rust programmer would write:

```rust
for s in &shapes {
    total += s.area();
```

Some of the design is already natural: a class tree is an `enum` with a `match` per method,
`toString` is `impl Display`, objects that are never changed have no `RefCell`.

## Why it looks the way it does

Java leaves things unsaid that Rust requires. Where the translator cannot prove the answer, it
takes the safe one, and the safe one is the noisy one:

| Java does not say… | so the translator assumes… | and writes |
|---|---|---|
| whether a reference can be null | it might be | `Option<T>`, `java::non_null(..)` at each use |
| who owns an object | anyone might share it | `Rc<..>`, `Rc<RefCell<..>>` |
| what a `List<Shape>` holds once generics are erased | any object | `Object` and a cast on every read |
| whether a call can throw | it might | `Result` and `unwrap_or_else(java::uncaught)` |
| whether `x + 1` can overflow | it might | `wrapping_add` |
| which nested class a name belongs to | (keeps the full Java name) | `ShapesAbstractShapeFields`, `new_string_f64_f64` |

## The principle: prove it, then simplify

The rule stays the one the translator has everywhere: never silently wrong. The general form stays
as the fallback; wherever the translator can prove a simpler form behaves the same, it writes the
simpler one. Each simplification is its own small step with its own proof and corpus programs, and
whatever cannot be proven keeps today's form, so correctness never depends on the analysis being
clever. Lost precision costs looks, never behaviour.

## Steps, in order of payoff

1. **Names and layout.** No analysis needed, and the most visible.
   - Nested classes become Rust modules: `shapes_abstract::Circle`, not `ShapesAbstractCircle`.
   - Overloaded methods and constructors get names that describe them instead of their parameter
     types (`new_string_f64_f64`), and the `…Fields` suffix on base-class structs goes away where
     no name clash needs it.
   - One file per package or top-level class instead of one `main.rs`.
   - The translated JDK goes into its own crate next to the program. The program's output becomes
     small and readable on its own, and the JDK crate is rebuilt only when what it depends on in
     the program changes, which also cuts compile time. (It depends on the program because the
     translation sees the whole program: the program's classes take part in the JDK's dispatch;
     see [large-projects.md](large-projects.md).)
2. **References that are never null** become plain values, with no `Option`. Most objects are
   created with `new` and never set to null; following that across fields, parameters, return
   values and collection elements removes most of the noise.
3. **Ownership.** An object that is never shared is stored directly, without `Rc`, and passed as
   `&T` or `&mut T`. This is the points-to work of thorough mode, including rustc as the checker
   for guessed ownership.
4. **Collections, loops and generics.**
   - `List<Shape>` becomes `Vec<Shape>`, with real element types and no casts
     ([lists-and-streams.md](lists-and-streams.md)).
   - Iterator loops and index loops that only read become `for x in &v`.
   - A Java `HashMap` becomes Rust's own only where its iteration order cannot reach the output,
     since the two iterate in different orders.
5. **Exceptions only where something catches them.** A method whose exceptions can only end the
   program needs no `Result`. Exceptions that are caught become ordinary error types, so a
   message is `e.message()` instead of `e.borrow().base.base.base.get_message()`.
6. **Arithmetic.** Plain `+` where value ranges prove the numbers stay in range. This needs the most
   analysis, so it comes last.

Steps 1, 2 and 4 are independent of each other and can run in parallel; 3 builds on the
points-to analysis that already exists for object identity.

## Measuring progress

The pass count says whether output is right; a second report says how natural it is. Per target
program and in total:

- the share of references typed `Option<..>`;
- the share of objects behind `Rc` and behind `RefCell`;
- the share of arithmetic that is `wrapping_*`;
- casts from `Object`;
- Rust lines per Java line, counting the program's own code separately from the JDK crate.

Next to that, three or four of the targets get a hand-written Rust version that is the goal. The
gap between the translation and that version is the to-do list, and closing it is the measure.

## Port mode (later)

Some Java behaviours can only be copied in clunky ways, because they show in the output and no
proof can remove them: the JDK's `HashMap` iteration order and `hashCode` values, integer
overflow that the program could reach, Java's text for `double`s, the exact text of exception
messages. The default mode copies them exactly and always will.

Port mode is for someone who wants to take the translated code over and maintain it as their own
Rust project, and who cares more about natural code than about matching Java byte for byte. It:

- writes natural Rust everywhere: Rust's `HashMap`, plain arithmetic (with Rust's overflow checks
  in debug builds as a safety net), `f64`'s own formatting, typed error enums without Java's
  messages, public signatures changed freely;
- **lists every place where it changed behaviour** in a report next to the code: the file and line,
  the Java behaviour, the Rust behaviour, and what kind of input would show the difference. Nothing
  changes without being listed, so it is still never *silently* different;
- builds on the relaxations in [thorough-mode.md](thorough-mode.md): port mode is all relaxations
  at once, where thorough mode lets the user grant them one at a time per program or package.

Order: port mode comes only after the exact mode is as clean as it can be made. Otherwise it
would hide clunkiness that a proof could have removed, and the report would be full of changes
that never needed to happen.

## Limits

- What is not proven stays in the general form, so a program that does genuinely Java-shaped
  things (objects shared everywhere, nulls passed around) keeps looking Java-shaped in exact mode.
- The deep analyses must scale to the JDK classes a program reaches; see the limits in
  [thorough-mode.md](thorough-mode.md).
- Some choices are a person's to make, not the translator's: crate boundaries, which errors a
  caller should handle, the public API of a library. Port mode makes them by default and lists
  them; the user can override them.
