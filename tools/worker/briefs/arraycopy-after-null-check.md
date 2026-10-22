# System.arraycopy on a parameter right after a null check

A known bug, found while the I/O streams were translated, and never fixed: a method that checks
an array parameter for null and throws, and then passes it to `System.arraycopy`, generates
`&b` on a value that is already a reference, so the Rust does not compile. The shape is:

```java
static void copy(byte[] b, int off, int len, byte[] dst) {
    if (b == null) throw new NullPointerException();
    System.arraycopy(b, off, dst, 0, len);
}
```

It may already be fixed by later changes. Find out first.

## Steps
1. Write that shape (and variants: `Objects.requireNonNull(b)` before the copy, the check on
   the destination instead, an `int[]` and a `char[]`, the parameter used again after the copy)
   as a corpus program corpus/features/ArrayCopyAfterNullCheck.java, called with real arrays
   and with null (catching the NullPointerException), and run it.
2. If everything matches Java, keep the program as a regression test and stop there.
3. If not, find where the generator writes the borrow for arraycopy's arguments (search
   `crates/translator/src/codegen` for `arraycopy`) and why the null check changes the
   parameter's form, and fix it there. Do not work around it in the corpus program.

## Deliverables
- The corpus program, matching Java.
- The fix, if one was needed, with a note in your report on the cause.

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`.
  `CORPUS_FILTER=arraycopyafternullcheck` runs only that corpus program.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Do not touch Unsafe, VarHandle, Charset, reflection, ClassLoader, liveness or exception
  analysis code: other agents are changing it now.
