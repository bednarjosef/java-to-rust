// The Minecraft server's `Shapes.joinIsNotEmpty`, which collision checks run for every entity
// that moves: it walks the merged coordinate lists of two shapes with an `IndexMerger` for each
// axis, nesting a lambda per axis, and a merger whose lists come in the other order swaps the
// indexes in a lambda that captures the consumer it was given. The mergers only call the lambdas.
public class MergedIndexes {
    interface IndexConsumer { boolean merge(int first, int second, int result); }

    interface IndexMerger {
        boolean forMergedIndexes(IndexConsumer consumer);
        int size();
    }

    static final class Identical implements IndexMerger {
        final int size;
        Identical(int size) { this.size = size; }
        public boolean forMergedIndexes(IndexConsumer consumer) {
            for (int i = 0; i < size; i++) {
                if (!consumer.merge(i, i, i)) return false;
            }
            return true;
        }
        public int size() { return size; }
    }

    static final class NonOverlapping implements IndexMerger {
        final int lower, upper;
        final boolean swap;
        NonOverlapping(int lower, int upper, boolean swap) { this.lower = lower; this.upper = upper; this.swap = swap; }
        public boolean forMergedIndexes(IndexConsumer consumer) {
            return swap ? forNonSwapped((a, b, r) -> consumer.merge(b, a, r)) : forNonSwapped(consumer);
        }
        private boolean forNonSwapped(IndexConsumer consumer) {
            for (int i = 0; i < lower; i++) {
                if (!consumer.merge(i, -1, i)) return false;
            }
            for (int i = 0; i < upper; i++) {
                if (!consumer.merge(lower - 1, i, lower + i)) return false;
            }
            return true;
        }
        public int size() { return lower + upper; }
    }

    static final class Shape {
        final boolean[] full;
        final int xs, ys, zs;
        Shape(int xs, int ys, int zs, int seed) {
            this.xs = xs; this.ys = ys; this.zs = zs;
            full = new boolean[xs * ys * zs];
            for (int i = 0; i < full.length; i++) full[i] = ((i * 31 + seed) % 7) == 0;
        }
        boolean isFull(int x, int y, int z) {
            return x >= 0 && y >= 0 && z >= 0 && x < xs && y < ys && z < zs && full[(x * ys + y) * zs + z];
        }
    }

    static boolean joinIsNotEmpty(IndexMerger mx, IndexMerger my, IndexMerger mz, Shape a, Shape b) {
        return !mx.forMergedIndexes((x1, x2, xr) -> my.forMergedIndexes((y1, y2, yr) -> mz.forMergedIndexes(
            (z1, z2, zr) -> !(a.isFull(x1, y1, z1) && b.isFull(x2, y2, z2)))));
    }

    public static void main(String[] args) {
        IndexMerger[] mergers = {
            new Identical(2), new NonOverlapping(1, 2, false), new NonOverlapping(2, 1, true), new Identical(3),
        };
        Shape[] shapes = new Shape[8];
        for (int i = 0; i < shapes.length; i++) shapes[i] = new Shape(3, 3, 3, i * 5 + 1);
        long joins = 0;
        long start = System.nanoTime();
        for (int tick = 0; tick < 3_000_000; tick++) {
            IndexMerger mx = mergers[tick & 3], my = mergers[(tick >>> 2) & 3], mz = mergers[(tick >>> 4) & 3];
            if (joinIsNotEmpty(mx, my, mz, shapes[tick & 7], shapes[(tick >>> 3) & 7])) joins++;
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1_000_000);
        System.out.println(joins);
    }
}
