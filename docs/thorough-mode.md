# Thorough mode (planned)

A slower translation mode that analyses the whole program at once and turns Java's design into
Rust's: owned data instead of shared objects, enums and matches instead of patterns built from
classes, iterators, derived traits, typed errors. It is what a release build is to a debug build:
the same program, more time spent to make it better.

The rule stays the same as everywhere else: every change must be proven to keep what the program
prints, returns and throws. Deeper analysis means more proofs, not more risk. Whatever cannot be
proven keeps today's representation, so the output stays correct.

It fits the existing design. Analyses fill the representation table and the code generator
follows it; thorough mode adds stronger analyses and design-level entries (per class or per
pattern instead of per value), not a second generator.

The speed side of this mode (the collected object model, points-to and escape analysis, and the
transformations they allow) is planned in docs/native-speed.md, which also replaces analysis
precision built from recorded runs: decisions come from the code alone. This doc keeps the idiom
side.

When: after the object model lands, because almost everything here works on its representation.
The array-ownership work (arrays as owned `Vec`s, returned and moved arrays) is a small first
version of the alias analysis for arrays only.

## Analyses

1. **Whole-program alias analysis (points-to).** For every reference, which allocation sites it
   can point to. Two names that never point to the same object let that object be owned instead
   of shared. Everything below builds on it. Closed-world points-to analysis over a program and
   the JDK is done in production by GraalVM's native image, so the scale is known to work.
2. **Shape analysis.** Whether a structure is a tree, a DAG or cyclic. A tree becomes
   `Option<Box<Node>>`, a list that holds its elements becomes `Vec<Node>`, and a cyclic graph
   becomes an arena (`Vec<Node>` plus indices).
3. **Effects across calls.** Which methods read and which write, and what. This decides `&self`
   or `&mut self` everywhere and removes most `RefCell`s and clones.
4. **What the program observes.** Whether iteration order, a hash value, identity or an exception
   message ever reaches output. If a `HashMap` is only used with `put` and `get`, its order cannot
   show, and it can be Rust's own `HashMap`. The same decides `ArrayList` as `Vec` and streams as
   iterators (docs/lists-and-streams.md) per allocation site.

## rustc as the proof checker for ownership

Ownership can be guessed aggressively and verified by Rust's borrow checker:

1. Emit the fully owned version: plain structs, moves and borrows, and no clones of objects.
2. If rustc accepts it, it is correct. With moves only, each object has exactly one name at any
   time; a Java alias that is used later shows up as "use after move" or a borrow conflict.
3. Where rustc reports an error, make that allocation site shared and try again.

This is sound only if the translator never inserts clones of objects, since a clone would split
one Java object into two. The cost is compile time, which this mode can afford.

## Design changes

Each is a separate transformation with its own proof and its own corpus programs, so they can be
added one at a time.

| Java design | Rust design | What must be proven |
|---|---|---|
| class hierarchy with virtual methods | `enum` with data, `match` | nothing beyond the closed world (the object model does this) |
| Visitor pattern (`accept`/`visit`) | one `match` over the enum | the visitors are the only users of `accept` |
| interface with one implementation | the concrete type, no trait | no other implementation exists |
| getters and setters | direct field access | they only read or write the field |
| Builder pattern | a struct literal | the builder does not escape and is used once |
| `null` or `-1` for "not found" | `Option<T>`, `Option<usize>` | every caller tests the sentinel before using the value |
| immutable value class, field-by-field `equals`/`hashCode` | `#[derive(Clone, Copy, PartialEq, Eq, Hash)]` | `equals` is field-by-field; the hash value never reaches output or ordering |
| `toString` | `impl Display` | none |
| `Comparable`, `Comparator` | `impl Ord`, `sort_by_key` | the comparison is consistent |
| a class implementing `Iterator` | `impl Iterator` | none |
| listener and callback interfaces | closures | one method, no identity use |
| index loop that sums or filters a list | iterator chain | the index is only used to read the element; arithmetic still wraps |
| defensive copy (`new ArrayList<>(x)`) | removed, or a move | the original is never used again |
| exception hierarchy | an error `enum` per method or package, and `?` | which exceptions each method can throw |
| static utility class, singleton | module functions, a plain struct | partly done |
| object graph with back pointers | arena with indices | shape analysis |
| many small objects in a list | `Vec<Struct>` stored contiguously, or one `Vec` per field | ownership and shape |

## Relaxations

Some differences are visible in output, so no proof can remove them:
- `HashMap` iteration order and `hashCode` values;
- integer overflow the program could reach;
- the exact text of exception messages.

Rust code would usually decide these differently. Relaxations are statements about one program
that the translator cannot prove and the user makes explicitly. Each one used is listed in the
translation report.

| Relaxation | Effect |
|---|---|
| iteration order of these maps does not matter | Rust's `HashMap` |
| arithmetic in this package never overflows | plain `+`, with overflow checks kept in debug builds as a safety net |
| exception messages are internal | typed error enums without message formatting |
| this is a program, not a library | public signatures may change freely |

Without relaxations the output matches Java exactly. With them, the user trades exactness they do
not need for more natural Rust, per program and in writing. That is where a person's judgement
comes back in: as a stated assumption, not a guess hidden in the code.

## Build order

1. Whole-program points-to analysis.
2. rustc-checked ownership speculation: large payoff for little work once (1) exists.
3. Observation analysis and library substitution: `HashMap`, `ArrayList`, streams.
4. Design changes, starting with the most common in real code: `Display`, derived traits,
   `Option` for sentinels, getters and setters, iterator chains, typed errors.
5. Shape analysis and arenas.
6. Relaxations and the report that lists them.
7. Data layout.

## Limits

- Alias and shape analysis precise enough for real code, while scaling to the hundreds of JDK
  classes a program reaches, is where most of the effort goes. Research tools that recover
  ownership for C translated to Rust show it works but loses precision on large programs. Lost
  precision costs speed and idiom, never correctness.
- A library, rather than a program, is a closed world only inside itself: its public API stays as
  written unless the "program, not a library" relaxation is given.
- Judgement calls such as crate structure, public API and which errors a caller should handle
  stay with the user, as relaxations or as hints.
