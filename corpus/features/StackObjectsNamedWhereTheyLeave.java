// Objects on the stack that are only read where they are made and leave for the heap on some
// paths: a position of a class with a subclass and one of a final class, kept by one place or
// by two (which must then keep the same object, also when it is read between them), an object
// that nothing reads before it leaves, one whose fields hold the only handles of an array and a
// string, one declared before the loop that keeps it, one that leaves from a nested block, a
// switch, a `return` and a call that copies its arguments first, and objects that are also used
// otherwise (passed to a call that only reads them, compared, read after they may have left),
// whose handle stays the slot's.
public class StackObjectsNamedWhereTheyLeave {
    static class Vec3i {
        private final int x, y, z;
        Vec3i(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        int getX() { return x; }
        int getY() { return y; }
        int getZ() { return z; }
    }

    static class BlockPos extends Vec3i {
        BlockPos(int x, int y, int z) { super(x, y, z); }
    }

    static final class MutablePos extends BlockPos {
        MutablePos() { super(-1, -1, -1); }
    }

    static final class Point {
        final int x;
        final boolean odd;
        final long weight;
        Point(int x) { this.x = x; this.odd = (x & 1) == 1; this.weight = x * 1000003L; }
        int x() { return x; }
    }

    static final class Marker {
        int touched;
    }

    static final class Named {
        final String name;
        final int length;
        Named(String name) { this.name = name; this.length = name.length(); }
    }

    // Its array is made with it and held by nothing else.
    static final class Crate {
        final int[] payload;
        final String label;
        final int n;
        Crate(int n) {
            this.n = n;
            this.payload = new int[] { n, n * 2, n * 3 };
            this.label = "crate " + n;
        }
    }

    static final class Keeper {
        Object last;
        int kept;
        int keep(Object o, int weight) { last = o; kept++; return weight + kept; }
    }

    static int seed = 7;
    static int[] scratch;
    static Object[] kept = new Object[512];
    static int count;
    static int reads;

    static BlockPos randomPos(int x0) {
        seed = seed * 3 + 1013904223;
        int v = seed >> 2;
        return new BlockPos(x0 + (v & 15), v >> 16 & 15, v >> 8 & 15);
    }

    static void keep(Object o) {
        if (count < kept.length) kept[count++] = o;
    }

    static Object keepAndGive(Object o) {
        keep(o);
        return o;
    }

    static int xOf(BlockPos p) { return p.getX(); }

    static int sea() { reads++; return 3; }

    static Keeper pick(Keeper[] keepers, int i) { return keepers[i % keepers.length]; }

    // The object leaves in the `return`.
    static Object made(int i) {
        Point p = new Point(i);
        if (p.x() % 3 == 0) {
            return keepAndGive(p);
        }
        return null;
    }

    public static void main(String[] args) {
        // A class with a subclass, from a factory: read by getters, kept by one place.
        long sum = 0;
        for (int i = 0; i < 3000; i++) {
            BlockPos pos = randomPos(i);
            sum += pos.getX() * 31L + pos.getY() - pos.getZ();
            if (pos.getY() == 3 && pos.getZ() == 5) keep(pos);
        }
        System.out.println("positions " + sum + " kept " + count);

        // Kept by two places in one turn: both keep the same object.
        int first = count;
        int same = 0;
        for (int i = 0; i < 400; i++) {
            BlockPos pos = randomPos(i);
            int before = count;
            int y = pos.getY();
            int z = pos.getZ();
            sum += pos.getX();
            if (y < 2) keep(pos);
            if (z < 2) keep(pos);
            if (count == before + 2 && kept[before] == kept[before + 1]) same++;
        }
        System.out.println("twice " + sum + " kept " + (count - first) + " same " + same);

        // Read again between the two places, through the handle, which stays the slot's until
        // the first of them moved the object.
        first = count;
        same = 0;
        for (int i = 0; i < 200; i++) {
            BlockPos pos = randomPos(i);
            int before = count;
            sum += pos.getX();
            if (pos.getY() < 2) keep(pos);
            if (pos.getZ() < 2) keep(pos);
            if (count == before + 2 && kept[before] == kept[before + 1]) same++;
        }
        System.out.println("between " + sum + " kept " + (count - first) + " same " + same);

        // A final class, read by a field and a getter; a boolean and a long field.
        long points = 0;
        int mark = count;
        for (int i = 0; i < 500; i++) {
            Point p = new Point(i);
            points += p.x + p.x() + (p.odd ? 1 : 0) + p.weight % 7;
            if (i % 97 == 0) {
                keep(p);
            }
        }
        System.out.println("points " + points + " kept " + (count - mark));

        // Nothing reads the object before it leaves.
        mark = count;
        for (int i = 0; i < 300; i++) {
            Marker m = new Marker();
            if (i % 71 == 0) keep(m);
        }
        System.out.println("markers " + (count - mark));

        // An object whose fields hold the only handles of an array and a string made with it:
        // until it is kept, what they refer to must stay alive through the collections that
        // the allocations in between run.
        mark = count;
        for (int i = 0; i < 400; i++) {
            Crate c = new Crate(i);
            scratch = new int[16 + i % 5];
            scratch[0] = i;
            if (i % 37 == 0) keep(c);
            scratch = new int[8];
        }
        System.out.println("crates " + (count - mark) + " " + scratch.length);

        // Declared before the loop that keeps it: every turn keeps the same object.
        Point outer = new Point(41);
        int outerSum = outer.x();
        mark = count;
        for (int i = 0; i < 9; i++) {
            if (i % 4 == 1) keep(outer);
        }
        System.out.println("outer " + outerSum + " " + (count - mark) + " " + (kept[mark] == kept[mark + 1]));

        // Leaving from a nested block, a switch and a labeled loop.
        mark = count;
        long nested = 0;
        turns:
        for (int i = 0; i < 200; i++) {
            Point p = new Point(i);
            nested += p.x();
            if (i % 2 == 0) {
                if (i % 26 == 0) {
                    keep(p);
                    continue turns;
                }
            }
            switch (i % 50) {
                case 7:
                    keep(p);
                    break;
                case 9:
                    nested += 1;
                    break;
                default:
                    break;
            }
        }
        System.out.println("nested " + nested + " kept " + (count - mark));

        // Leaving in a `return`.
        int returned = 0;
        for (int i = 0; i < 20; i++) {
            if (made(i) != null) returned++;
        }
        System.out.println("returned " + returned);

        // Leaving by a call that copies its arguments first: the receiver may be null and an
        // argument has an effect.
        Keeper[] keepers = { new Keeper(), new Keeper() };
        long weights = 0;
        for (int i = 0; i < 60; i++) {
            Point p = new Point(i);
            int x = p.x();
            if (x % 10 == 0) {
                Keeper k = pick(keepers, i);
                weights += k.keep(p, sea());
            }
        }
        System.out.println("copied " + weights + " " + reads + " " + keepers[0].kept + " "
            + ((Point) keepers[0].last).x);

        // A field that is not a number is read through the handle, which stays the slot's.
        mark = count;
        int letters = 0;
        for (int i = 0; i < 40; i++) {
            Named n = new Named("n" + i);
            letters += n.length + n.name.length();
            if (i % 13 == 0) keep(n);
        }
        System.out.println("named " + letters + " kept " + (count - mark));

        // Used otherwise before it leaves: given to a call that only reads it, compared with a
        // kept one, and read after a place where it may have left.
        mark = count;
        long others = 0;
        BlockPos fixed = new MutablePos();
        keep(fixed);
        for (int i = 0; i < 300; i++) {
            BlockPos pos = randomPos(i);
            others += xOf(pos);
            if (pos.getY() == 9) keep(pos);
            if (pos == fixed) others += 1000;
            others += pos.getZ();
        }
        System.out.println("others " + others + " kept " + (count - mark));

        long all = 0;
        for (int i = 0; i < count; i++) {
            Object o = kept[i];
            if (o instanceof MutablePos) all += 5;
            if (o instanceof BlockPos b) all += b.getX() * 3L + b.getY() * 5L + b.getZ() * 7L;
            if (o instanceof Point p) all += p.x + (p.odd ? 2 : 0) + p.weight;
            if (o instanceof Marker m) all += 11 + m.touched;
            if (o instanceof Named n) all += n.length;
            if (o instanceof Crate crate) {
                all += crate.payload[0] + crate.payload[1] + crate.payload[2] + crate.n + crate.label.length();
            }
        }
        System.out.println("all " + count + " " + all);
    }
}
