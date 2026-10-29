// Fields read through a type of several classes, as the Minecraft server's noise chunk reads the
// fields of its density buffers: the reads go through the type's `base()`, one load whatever the
// class. Translated into several crates (`--crates`), `base()` is small enough to stay in the
// header, where the callers in every crate inline it, rather than call a body crate. The sum is a
// `long`: a `double` one, once `run` is inlined into `main`, was kept on the stack, and that
// store and load on every add hid the reads.
public class BaseFields {
    abstract static class Cell {
        final long weight;
        final int count;

        Cell(long weight, int count) {
            this.weight = weight;
            this.count = count;
        }

        abstract double scale();
    }

    static final class Plain extends Cell {
        Plain(long weight, int count) { super(weight, count); }
        double scale() { return 1.0; }
    }

    static final class Doubled extends Cell {
        Doubled(long weight, int count) { super(weight, count); }
        double scale() { return 2.0; }
    }

    static final class Halved extends Cell {
        Halved(long weight, int count) { super(weight, count); }
        double scale() { return 0.5; }
    }

    static final class Negated extends Cell {
        final int sign;
        Negated(long weight, int count) { super(weight, count); this.sign = -1; }
        double scale() { return sign; }
    }

    static long run(Cell[] cells, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < cells.length; i++) {
                Cell c = cells[i];
                sum += c.weight * c.count;
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        Cell[] cells = new Cell[1024];
        for (int i = 0; i < cells.length; i++) {
            long w = (i % 17) * 3L;
            int n = i % 5 + args.length;
            cells[i] = switch (i % 4) {
                case 0 -> new Plain(w, n);
                case 1 -> new Doubled(w, n);
                case 2 -> new Halved(w, n);
                default -> new Negated(w, n);
            };
        }
        long check = 0;
        for (int warm = 0; warm < 2; warm++) {
            check += run(cells, 2000);
        }
        long start = System.nanoTime();
        long result = run(cells, 200000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("base fields result " + result + " " + check);
        System.err.println("kernel-ms " + elapsed);
    }
}
