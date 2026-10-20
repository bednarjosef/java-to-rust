# Arrays

Translate Java arrays: today `lower.rs` rejects them with "arrays not supported yet".

## Scope
- Array types of every supported element type, including arrays of arrays: `int[]`, `long[]`,
  `double[]`, `boolean[]`, `String[]`, `int[][]`. Locals, parameters, return values.
- Creation: `new int[n]` (zero-filled; `false`, `0.0`, and `null` for `String[]`, which you may
  reject as "null not supported yet" until the null feature lands), `new int[n][m]`,
  initializers `{1, 2, 3}` and `new int[] {1, 2}`.
- `a[i]` reads and writes, `a[i] += x` and every compound assignment, `a[i]++` and `--`,
  `a.length`, for-each over arrays (`for (int x : a)`), `main(String[] args)` using `args`.
- Java semantics: an index out of range throws `ArrayIndexOutOfBoundsException` (exit code 1),
  `new int[-1]` throws `NegativeArraySizeException`, arrays are references (aliasing is visible:
  `int[] b = a; b[0] = 5;` changes `a[0]`), and evaluation order (`a[i++] = i`).
- `java.util.Arrays.toString`, `Arrays.fill`, `Arrays.sort` for primitive arrays, and
  `Arrays.copyOf`, recognised the same way `lower.rs` recognises `Math.max` (by `symbol.owner`).

## Translation
- Faithful form: arrays are shared, so `Rc<RefCell<Vec<T>>>` (or a small runtime type around
  it, in `crates/runtime/src/array.rs`). Bounds errors must exit with code 1, not 101: either a
  runtime index helper that calls `java::throw`, or a panic hook that the generated `main`
  installs and that turns panics into Java's exit code (pick one and say why in your notes).
- Idiomatic form, by analysis (`src/analysis/arrays.rs`): an array that never has two names at
  once (never copied to a second variable, field or return value while the first is still used)
  becomes a plain `Vec<T>`, passed as `&[T]` to methods that only read it and `&mut [T]` to
  methods that write it. Indexing a `Vec` with `a[i as usize]` is the idiomatic form; use the
  range analysis (`src/analysis/ranges.rs`, extend it for `a.length` and array elements) to
  drop casts or checks only where proven.
- Counting loops over `a.length` (`for (int i = 0; i < a.length; i++)` reading only `a[i]`)
  should become `for x in &a` / `for x in a.iter_mut()` where the index is not otherwise used.

## Deliverables
- Array types, creation, initializers, indexing, `length`, compound assignment and `++` on elements
- Faithful translation with exceptions matching Java's exit code
- Aliasing analysis: unaliased arrays become `Vec<T>`, `&[T]`, `&mut [T]`
- For-each loops over arrays and index loops turned into iterator loops where safe
- `Arrays.toString`, `fill`, `sort`, `copyOf`; `args` in `main`
- Corpus programs: `corpus/features/Arrays.java`, `corpus/features/ArrayAliasing.java`, `corpus/features/ArrayExceptions.java`, and `corpus/programs/Sieve.java` (a sieve of Eratosthenes plus a sort)
- Shape tests in `tests/translation.rs` for the idiomatic forms
- README "Current state" updated
