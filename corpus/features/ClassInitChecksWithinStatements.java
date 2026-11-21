// Classes that one operand of an expression initializes and a later operand uses again: a call's
// later arguments and the call itself, the right operand of an operator, past `&&`, and the
// methods of an enum whose fields are tables, called after one call checked it. The static blocks
// print in Java's order whichever check the translation leaves out, and a use after an operand
// that may not run (a branch of `?:`, what follows `&&`) still initializes its class.
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

    enum Axis {
        X,
        Y,
        Z
    }

    // Its fields are tables that its initialization fills, so its methods check it.
    enum Dir {
        DOWN(Axis.Y, 0, -1, 0),
        UP(Axis.Y, 0, 1, 0),
        NORTH(Axis.Z, 0, 0, -1),
        EAST(Axis.X, 1, 0, 0);

        static final int COUNT = Log.note("Dir", 4);
        final Axis axis;
        final int x;
        final int y;
        final int z;

        Dir(Axis axis, int x, int y, int z) {
            this.axis = axis;
            this.x = x;
            this.y = y;
            this.z = z;
        }

        int stepX() {
            return x;
        }

        int stepY() {
            return y;
        }

        int stepZ() {
            return z;
        }

        int length() {
            return Math.abs(stepX()) + Math.abs(stepY()) + Math.abs(stepZ()) + COUNT;
        }
    }

    static final class Pos {
        final int x;
        final int y;
        final int z;

        Pos(int x, int y, int z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        Pos relative(Dir d) {
            return new Pos(x + d.stepX(), y + d.stepY(), z + d.stepZ());
        }
    }

    static int walk(Dir[] dirs, int n) {
        Pos p = new Pos(0, 0, 0);
        for (int i = 0; i < n; i++) {
            Dir d = dirs[i % dirs.length];
            p = p.relative(d);
            int a = d.stepX();
            p = new Pos(p.x + a, p.y + d.stepY(), p.z + d.length() + d.axis.ordinal());
        }
        return p.x * 100 + p.y * 10 + p.z;
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
        System.out.println("walk " + walk(Dir.values(), 10));
    }
}
