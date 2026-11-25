# java-to-rust

Translates Java programs into Rust programs that print exactly what the Java program prints.

It is a compiler, not an LLM: javac parses and type-checks the source, a set of program analyses
works out what each value can and cannot do, and a code generator writes Rust from those facts.
Where the analyses prove it safe, the output is plain Rust (owned structs, `Vec`, plain
arithmetic, no null checks); everywhere else it falls back to a faithful form that keeps Java's
behaviour.

It handles real programs, not just small examples:

- **Its test suite:** over a thousand Java programs, each checked against the JVM's output.
- **javac itself:** the translated compiler writes the same class files as the real one, byte
  for byte.
- **A Minecraft server:** the translated server runs. It starts in under a second and uses a
  third of the JVM's memory.

## Example

```java
public class Shapes {
    interface Shape { double area(); }
    record Circle(double r) implements Shape { public double area() { return Math.PI * r * r; } }
    record Square(double side) implements Shape { public double area() { return side * side; } }

    public static void main(String[] args) {
        Shape[] shapes = { new Circle(1.5), new Square(2) };
        double total = 0;
        for (Shape s : shapes) total += s.area();
        System.out.printf("total area: %.2f%n", total);
    }
}
```

becomes (abridged):

```rust
struct ShapesCircle {
    r: f64,
}

impl ShapesCircle {
    fn area(&self) -> f64 {
        3.141592653589793 * self.r * self.r
    }
}

fn main() {
    let shapes: Vec<ShapesShape> = vec![
        ShapesShape(java::Thin::pointer::<Arc<ShapesCircle>>(2, Arc::new(ShapesCircle__new(1.5)))),
        ShapesShape(java::Thin::pointer::<Arc<ShapesSquare>>(3, Arc::new(ShapesSquare__new(2.0)))),
    ];
    let mut total = 0.0;
    for s in shapes.iter().cloned() {
        total += ShapesShape__area(s);
    }
    java::print_format(java::wtf8("total area: %.2f%n"), &[java::Arg::Double(total)]);
}
```

What Rust lacks (Java's number formatting, strings, threads and monitors, a garbage collector,
the JDK's native methods) comes from the runtime crate in `crates/runtime`.

## Build and run

You need a Rust toolchain and a JDK (21 or newer) with `javac` and `java` on `PATH`.

```bash
cargo run --release -p java-to-rust -- Program.java              # print the translation
cargo run --release -p java-to-rust -- Program.java --crate out/ # write a Cargo project
cd out && cargo run --release
```

Larger programs are translated whole, together with the JDK's own library sources:

```bash
cargo run --release -p java-to-rust -- --main com.example.Main --source src/ \
    --library jdk-src/java.base --crate out/
```

All options are in [docs/usage.md](docs/usage.md).

## Test

```bash
scripts/check.sh
```

Every test program is run on Java and, translated, as Rust; output and exit code must match
exactly. Java's output is the expected result, so no expectations are written by hand. See
[docs/testing.md](docs/testing.md) for the test layout, the fuzzer and the benchmarks.

## Documentation

- [What Java is supported](docs/supported-java.md), per language and library area
- [Architecture](docs/architecture.md): the pipeline and each stage
- [Object model](docs/object-model.md), [closures and nested classes](docs/closures-and-nested-classes.md),
  [exceptions](docs/exceptions.md), [threads](docs/multi-threading.md),
  [the garbage collector](docs/collector.md)
- [Performance](docs/performance.md) against the JVM
- [Plans](docs/native-speed.md) for faster output, [a thorough mode](docs/thorough-mode.md) and
  [more natural Rust](docs/natural-rust.md)

Everything else is in [docs/](docs/).

## License

GPL-3.0-or-later; see [LICENSE](LICENSE).
