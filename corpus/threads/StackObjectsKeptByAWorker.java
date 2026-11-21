// Objects on the stack of classes that threads share, only read where they are made and kept
// on some paths, on a worker thread as a server's ticks run: a position of a class with a
// subclass that one place or two keep (two keep the same object), one of a final class, an
// object that nothing reads before it is kept, and one whose fields hold the only handles of an
// array and a string. The kept objects are then read on the main thread.
public class StackObjectsKeptByAWorker {
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
        int moves;
        MutablePos() { super(-1, -1, -1); }
    }

    static final class Point {
        final int x;
        final long weight;
        Point(int x) { this.x = x; this.weight = x * 1000003L; }
        int x() { return x; }
    }

    static final class Marker {
        int touched;
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

    static final class Level {
        int randValue = 1;
        int[] scratch;
        final Object[] kept = new Object[512];
        int count;
        long sum;
        int same;

        BlockPos randomPos(int x0) {
            randValue = randValue * 3 + 1013904223;
            int v = randValue >> 2;
            return new BlockPos(x0 + (v & 15), v >> 16 & 15, v >> 8 & 15);
        }

        synchronized void keep(Object o) {
            if (count < kept.length) kept[count++] = o;
        }

        void tick(int turns) {
            for (int i = 0; i < turns; i++) {
                BlockPos pos = randomPos(i);
                sum += pos.getX() * 31L + pos.getY() - pos.getZ();
                if (pos.getY() == 3 && pos.getZ() == 5) keep(pos);
            }
            for (int i = 0; i < turns / 8; i++) {
                BlockPos pos = randomPos(i);
                int before = count;
                int y = pos.getY();
                int z = pos.getZ();
                sum += pos.getX();
                if (y < 2) keep(pos);
                if (z < 2) keep(pos);
                if (count == before + 2 && kept[before] == kept[before + 1]) same++;
            }
            for (int i = 0; i < turns / 4; i++) {
                Point p = new Point(i);
                sum += p.x + p.x() + p.weight % 7;
                if (i % 97 == 0) keep(p);
            }
            for (int i = 0; i < turns / 8; i++) {
                Marker m = new Marker();
                if (i % 71 == 0) keep(m);
            }
            // What the crate's fields refer to must stay alive until it is kept, through the
            // collections that the allocations in between run.
            for (int i = 0; i < turns / 6; i++) {
                Crate c = new Crate(i);
                scratch = new int[16 + i % 5];
                scratch[0] = i;
                if (i % 37 == 0) keep(c);
                scratch = new int[8];
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Level level = new Level();
        level.keep(new MutablePos());
        Thread worker = new Thread(() -> level.tick(2400));
        worker.start();
        worker.join();
        long all = 0;
        for (int i = 0; i < level.count; i++) {
            Object o = level.kept[i];
            if (o instanceof MutablePos m) all += 5 + m.moves;
            if (o instanceof BlockPos b) all += b.getX() * 3L + b.getY() * 5L + b.getZ() * 7L;
            if (o instanceof Point p) all += p.x + p.weight;
            if (o instanceof Marker m) all += 11 + m.touched;
            if (o instanceof Crate crate) {
                all += crate.payload[0] + crate.payload[1] + crate.payload[2] + crate.n + crate.label.length();
            }
        }
        System.out.println("kept " + level.count + " same " + level.same + " sum " + level.sum + " all " + all);
    }
}
