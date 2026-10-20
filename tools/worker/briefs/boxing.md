# Boxing and unboxing

Translate `Integer`, `Long`, `Double`, `Boolean`, `Character`, `Byte`, `Short` and `Float`
values; today `lower.rs` rejects the types.

## Scope
- Boxed locals, parameters, return values and fields; autoboxing (`Integer x = 5;`, passing an
  `int` to an `Integer` parameter) and unboxing (`int y = x;`, `x + 1`, `x < y`, `x++` on an
  `Integer` local, which unboxes, adds and boxes again).
- `null` boxed values: `Integer x = null;`, `x == null`, and unboxing `null` throws
  `NullPointerException` (exit 1) via `java::non_null`, as the null feature does for strings.
- Static methods: `Integer.valueOf(int)`, `Integer.parseInt(String)` (a bad string throws
  `NumberFormatException`, exit 1, with Java's message on stderr only), `Integer.toString(int)`,
  `Integer.compare`, `Integer.max/min/sum`, `Integer.MAX_VALUE` (already done for primitives),
  `Long.parseLong`, `Double.parseDouble` (Java's grammar, which differs from Rust's: `"1d"`,
  `" 2 "` trimmed, `"0x1p3"`), `Boolean.parseBoolean`, `Character.getNumericValue`; instance
  methods `x.intValue()`, `x.doubleValue()`, `x.equals(y)`, `x.compareTo(y)`, `x.hashCode()`
  (`Integer.hashCode` is the value; `Long`'s is `(int)(v ^ (v >>> 32))`; `Double`'s uses its
  bits), `x.toString()`, and printing and string concatenation (`"n=" + x` prints `null` for
  null).
- Identity: `==` between two boxed values compares references in Java. `valueOf` (and
  autoboxing) returns cached objects for -128..127 (`Integer`, `Long`, `Short`, `Byte`,
  `Character` up to 127, both `Boolean`s), so `Integer a = 100, b = 100; a == b` is true and
  with 1000 it is false; `a.equals(b)` is true for both. `==` between a boxed and a primitive
  unboxes.

## Translation
- Idiomatic form, by analysis (`src/analysis/boxing.rs`): a boxed variable that is never
  compared with `==`/`!=` against another boxed value is just its value: `i32` when it can
  never be null (the nullness analysis in `analysis/nullness.rs` already decides this for
  strings; extend it to boxed types), `Option<i32>` when it can.
- Faithful form, for boxed values whose identity is observed by `==`: a runtime type in
  `crates/runtime/src/boxed.rs`, for example `java::Boxed<T>` holding an `Rc<T>`, with
  `java::box_int(v)` returning a shared cached object for -128..127 and a new one otherwise,
  and `java::same(&a, &b)` comparing pointers (`Rc::ptr_eq`). Nullable: `Option<Boxed<T>>`.
- Parsing and formatting helpers go in the runtime with unit tests whose expected values come
  from running Java (write a Java snippet, run it with `java File.java`, paste its output).
- Recognize the methods by `symbol.owner` the way `lower.rs` recognizes `Math` and `Character`
  methods; put the lowering in `lower/boxing.rs` and the code generation in `codegen/boxing.rs`.

## Deliverables
- Boxed types as locals, parameters, returns and fields, with autoboxing and unboxing
- Null boxed values and NullPointerException on unboxing null
- parseInt, parseLong, parseDouble, parseBoolean with Java's exceptions, valueOf, toString,
  compare, equals, compareTo, hashCode, intValue and friends
- `==` identity with the valueOf cache, faithful form
- Analysis: boxed values without identity comparisons are plain `i32` / `Option<i32>`
- Corpus programs: `corpus/features/Boxing.java`, `corpus/features/BoxedIdentity.java`,
  `corpus/features/Parsing.java` (exits 1 on a bad number at the end), and
  `corpus/programs/WordLengths.java` (parses numbers from strings and sums boxed values)
- Shape tests in `tests/translation.rs` for plain `i32`, `Option<i32>` and the boxed form
