// Classes that one operand of an expression initializes and a later operand uses again: a call's
// later arguments and the call itself, the right operand of an operator, past `&&`. The static
// blocks print in Java's order whichever check the translation leaves out, and a use after an
// operand that may not run (a branch of `?:`, what follows `&&`) still initializes its class.
public class ClassInitChecksWithinStatements {
    static class Log {
        static int count;

        static int note(String who, int value) {
            count++;
            System.out.println("init " + who + " (" + count + ")");
            return value;
        }
    }

    static class Sections {
        static final int SHIFT = Log.note("Sections", 4);

        static int toSection(int block) {
            return block >> SHIFT;
        }

        static long pack(int x, int z) {
            return ((long) x << 32) | (z & 0xffffffffL);
        }
    }

    static class Grid {
        static int cells = Log.note("Grid", 1);

        static int count(int a, int b) {
            cells += a + b;
            return cells;
        }
    }

    static class Gate {
        static int limit = Log.note("Gate", 2);

        static boolean open(int v) {
            return v > limit;
        }
    }

    static class Branchy {
        static int base = Log.note("Branchy", 3);

        static int value(int v) {
            return v * base;
        }
    }

    static class Box {
        static int made = Log.note("Box", 0);
        final long key;

        Box(long a, long b) {
            made++;
            key = a * 31 + b;
        }
    }

    static long key(int x, int z) {
        return Sections.pack(Sections.toSection(x), Sections.toSection(z));
    }

    static int sum(int x, int z) {
        return Grid.count(x, 1) + Grid.count(z, 2) * Grid.cells;
    }

    static boolean gates(int x) {
        // With x at most 100 the first `open` does not run, and the second initializes Gate.
        return (x > 100 && Gate.open(x)) | Gate.open(x + 3);
    }

    static boolean both(int x) {
        return Gate.open(x) && Gate.open(x - 1);
    }

    static int choose(boolean b, int x) {
        // With b false the first `value` does not run, and the second initializes Branchy.
        return (b ? Branchy.value(x) : 0) + Branchy.value(x);
    }

    static long boxed(int x) {
        return new Box(Sections.toSection(x), Box.made).key + Box.made;
    }

    public static void main(String[] args) {
        System.out.println("start");
        System.out.println("choose " + choose(false, 5) + " " + choose(true, 5));
        System.out.println("gates " + gates(1) + " " + gates(200) + " " + both(4) + " " + both(1));
        long total = 0;
        for (int i = 0; i < 4; i++) {
            total += key(i * 16, i * 32 + 5);
            total += sum(i, i + 1);
            total += boxed(i * 40);
        }
        System.out.println("total " + total + " made " + Box.made + " cells " + Grid.cells);
    }
}
