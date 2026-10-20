# Lists and streams as Rust values (planned)

Most of `java.util` is translated from the JDK's own source, so it behaves exactly like Java.
Two exceptions are planned, because Rust's standard library has a direct equivalent and the
translated code would read like Java and run slower: `ArrayList` as a `Vec`, and sequential
streams as Rust iterators. `HashMap` and the other hash collections stay translated: their
iteration order shows in printed output, and Rust's differs on every run.

When: after the object model, lambdas and for-each over `Iterable` land. Both need boxing in
generics and closures.

## ArrayList

Each allocation site gets one of three representations, as with every other value:

| Allocation site | Representation |
|---|---|
| owned (not aliased), and never modified while iterated | plain `Vec<T>` |
| modified while iterated | a `java::ArrayList<T>` wrapper: `Vec<T>` plus `mod_count` |
| uses `subList`, list iterators, `iterator().remove()` or other rare API | the JDK's `ArrayList`, translated |

- **ConcurrentModificationException.** In a for-each whose body provably cannot change the
  list, Java cannot throw and Rust's `for x in &v` is exact; the borrow checker forbids the
  change anyway. Otherwise the wrapper counts adds, removes and clears like Java's `modCount`,
  and the loop checks it before each step like Java's iterator. That includes the quirk that
  removing the second-to-last element ends the loop without throwing, because `hasNext()`
  compares the cursor with the new size first.
- **Sharing.** A list is an object, so `a = b` aliases it. A plain `Vec` needs the same
  ownership proof as arrays (`analysis/arrays.rs` and its extension to returned and moved
  arrays).
- **Boxed elements.** `List<Integer>` holds nullable `Integer` objects. `==` on them compares
  identity, which is true only inside the -128 to 127 cache, and `remove(int)` is a different
  call from `remove(Object)`. A `Vec<i32>` needs nullness and boxing proofs; otherwise the
  elements stay boxed.
- **Exact API behaviour.** `toString` is `[1, 2, 3]`, plus `equals`, `hashCode`, and
  `IndexOutOfBoundsException` with Java's messages.

## Sequential streams

`list.stream().filter(..).map(..).collect(..)` and the like become Rust iterator chains:
`iter().filter(..).map(..).collect()`, `sum`, `count`, `anyMatch` as `any`, `forEach` as
`for_each`. Both are lazy and both short-circuit in the same places, so side effects inside the
lambdas run in the same order. `Collectors.toList()` collects into the list representation
above. `groupingBy` and `toMap` collect into the translated `HashMap`, so iteration order stays
Java's. Parallel streams, and pipelines whose elements a method cannot map one to one, use the
translated JDK stream code.
