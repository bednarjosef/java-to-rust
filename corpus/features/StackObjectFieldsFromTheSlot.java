// Objects on the stack whose fields are read, directly or by getters, right where the object is
// made: a small immutable point per iteration, a position with a mutable subclass that a factory
// makes and that sometimes leaves for the heap, and objects whose fields change after a read, by a
// call, by an assignment or in a loop, whose later reads must see the change.
public class StackObjectFieldsFromTheSlot {
    static final class Vec3 {
        final double x, y, z;
        Vec3(double x, double y, double z) { this.x = x; this.y = y; this.z = z; }
    }

    static class Vec3i {
        private int x, y, z;
        Vec3i(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        int getX() { return x; }
        int getY() { return y; }
        int getZ() { return z; }
        protected void setX(int x) { this.x = x; }
    }

    static class BlockPos extends Vec3i {
        BlockPos(int x, int y, int z) { super(x, y, z); }
        BlockPos above() { return new BlockPos(getX(), getY() + 1, getZ()); }
        // Not a getter: it writes a field before it reads it.
        int shiftX() { setX(getX() + 100); return getX(); }
    }

    static final class MutablePos extends BlockPos {
        MutablePos() { super(0, 0, 0); }
        MutablePos set(int x) { setX(x); return this; }
    }

    static final class Counter {
        int n;
        Counter(int n) { this.n = n; }
        void bump() { n++; }
        int get() { return n; }
    }

    // Some counter is kept, so that counters are objects with handles, not plain values.
    static Counter lastCounter = new Counter(-1);
    static int seed = 7;
    static BlockPos[] kept = new BlockPos[16];
    static int count;

    static BlockPos randomPos(int x0) {
        seed = seed * 3 + 1013904223;
        int v = seed >> 2;
        return new BlockPos(x0 + (v & 15), v >> 16 & 15, v >> 8 & 15);
    }

    // Takes a mutable position as a `BlockPos`, which makes `BlockPos` a type of several classes,
    // whose handles carry the class in their top bits.
    static int xOf(BlockPos p) { return p.getX(); }

    static void keep(BlockPos p) {
        if (count < kept.length) kept[count++] = p;
    }

    public static void main(String[] args) {
        // A point per iteration, read right after it is made.
        double total = 0;
        for (int i = 0; i < 2000; i++) {
            Vec3 v = new Vec3(i, i * 0.5, 2);
            total += v.x + v.y * v.z;
        }
        System.out.println("points " + total);

        // Positions from a factory, read by getters, some kept in the heap.
        long sum = 0;
        for (int i = 0; i < 2000; i++) {
            BlockPos pos = randomPos(i);
            sum += pos.getX() * 31L + pos.getY() - pos.getZ();
            if (pos.getY() == 3) keep(pos);
            BlockPos up = pos.above();
            sum += up.getY() * 7L;
        }
        System.out.println("positions " + sum + " kept " + count);
        for (int i = 0; i < count; i++) {
            System.out.print(kept[i].getX() + "," + kept[i].getY() + "," + kept[i].getZ() + " ");
        }
        System.out.println();

        // A mutable subclass's field written by its setter.
        MutablePos m = new MutablePos();
        int ms = 0;
        for (int i = 0; i < 10; i++) {
            ms += m.set(i * 3).getX();
        }
        System.out.println("mutable " + ms + " " + xOf(m));

        // Fields that change after a read: the later reads see the change.
        long changed = 0;
        for (int i = 0; i < 100; i++) {
            BlockPos p = new BlockPos(i, i + 1, i + 2);
            int before = p.getX();
            int shifted = p.shiftX();
            changed += before + shifted + p.getX();
        }
        System.out.println("shifted " + changed);

        long counted = 0;
        for (int i = 0; i < 50; i++) {
            Counter c = new Counter(i);
            counted += c.n;
            for (int j = 0; j < 4; j++) {
                counted += c.n * 10L + c.get();
                c.bump();
            }
            c.n += 1000;
            counted += c.n + c.get();
        }
        System.out.println("counted " + counted);

        // A read in a switch expression, before and after a write of the field.
        int switched = 0;
        for (int i = 0; i < 30; i++) {
            Counter c = new Counter(i);
            switched += switch (i % 3) {
                case 0 -> c.get();
                case 1 -> { c.bump(); yield c.n; }
                default -> c.n * 2;
            };
            switched += c.get();
        }
        System.out.println("switched " + switched + " " + lastCounter.get());
    }
}
