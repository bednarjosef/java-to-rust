// In a class whose constructor starts a thread, a field stays fixed (a plain load) when every
// write of it comes before `this` can have left the constructor: the enclosing instance of an
// inner class, which javac writes before super(..), field initializers, and assignments before
// the first statement that passes, stores, captures or calls a method on `this`. A field written
// after that point, after a superclass constructor that lets `this` leave, or after this(..),
// stays a shared field. The loop over neighbors walks chains of such fields, as the Minecraft
// server's chunk distance trackers do.
public class FieldsWrittenBeforeThisLeaves {
    static final Object[] REGISTRY = new Object[8];
    static int registered;

    static void register(Object o) {
        REGISTRY[registered++] = o;
    }

    static long pack(int x, int z) {
        return (long) x & 4294967295L | ((long) z & 4294967295L) << 32;
    }

    static int mix(long k) {
        long h = k * 0x9e3779b97f4a7c15L;
        h ^= h >>> 32;
        return (int) (h ^ h >>> 16);
    }

    // Open addressing on long keys, 0 meaning empty, as fastutil's sets and maps do.
    static final class LongSet {
        long[] key;
        int mask;
        int size;

        LongSet(int capacity) {
            key = new long[capacity];
            mask = capacity - 1;
        }

        boolean contains(long k) {
            final long[] key = this.key;
            int pos = mix(k) & mask;
            while (key[pos] != 0) {
                if (key[pos] == k) {
                    return true;
                }
                pos = pos + 1 & mask;
            }
            return false;
        }

        void add(long k) {
            if (contains(k)) {
                return;
            }
            if (++size * 2 > key.length) {
                long[] old = key;
                key = new long[old.length * 2];
                mask = key.length - 1;
                size = 1;
                for (long o : old) {
                    if (o != 0) {
                        add(o);
                    }
                }
            }
            int pos = mix(k) & mask;
            while (key[pos] != 0) {
                pos = pos + 1 & mask;
            }
            key[pos] = k;
        }
    }

    static final class LongMap {
        long[] key;
        Object[] value;
        int mask;

        LongMap(int capacity) {
            key = new long[capacity];
            value = new Object[capacity];
            mask = capacity - 1;
        }

        Object get(long k) {
            final long[] key = this.key;
            int pos = mix(k) & mask;
            while (key[pos] != 0) {
                if (key[pos] == k) {
                    return value[pos];
                }
                pos = pos + 1 & mask;
            }
            return null;
        }

        void put(long k, Object v) {
            int pos = mix(k) & mask;
            while (key[pos] != 0 && key[pos] != k) {
                pos = pos + 1 & mask;
            }
            key[pos] = k;
            value[pos] = v;
        }
    }

    static final class Holder {
        final int ticketLevel;

        Holder(int ticketLevel) {
            this.ticketLevel = ticketLevel;
        }
    }

    static void runAside(Runnable r) {
        Thread t = new Thread(r);
        t.start();
        try {
            t.join();
        } catch (InterruptedException e) {
            throw new RuntimeException(e);
        }
    }

    // Its constructor lets `this` leave: a subclass's fields are all written after that.
    static class Tracker {
        final int levelCount;

        Tracker(int levelCount) {
            this.levelCount = levelCount;
            register(this);
        }
    }

    static final class Grid {
        // Written by initializers, before the constructor's body lets `this` leave.
        final LongSet toDrop = new LongSet(16);
        final LongMap updating = new LongMap(256);
        final int radius;
        final Thread filler;
        // Written after `this` left (the inner class's object holds it).
        final Distances distances;
        int late;

        Grid(int radius) {
            this.radius = radius;
            this.filler = new Thread(this::fill);
            this.filler.start();
            this.distances = new Distances();
            this.late = radius * 7;
        }

        void fill() {
            for (int x = -radius; x <= radius; x++) {
                for (int z = -radius; z <= radius; z++) {
                    if ((x + z & 3) == 0) {
                        toDrop.add(pack(x, z));
                    }
                    updating.put(pack(x, z), new Holder(Math.abs(x) + Math.abs(z)));
                }
            }
        }

        final class Distances extends Tracker {
            // Written after Tracker's constructor let `this` leave.
            final long origin;

            Distances() {
                super(33);
                origin = pack(0, 0);
            }

            boolean isChunkToRemove(long node) {
                return toDrop.contains(node);
            }

            Holder getChunk(long node) {
                return (Holder) updating.get(node);
            }

            int getLevel(long node) {
                if (!isChunkToRemove(node)) {
                    Holder chunk = getChunk(node);
                    if (chunk != null) {
                        return chunk.ticketLevel;
                    }
                }
                return levelCount - 1;
            }

            int computedLevel(long node, int x, int z) {
                int computed = levelCount - 1;
                for (int dx = -1; dx <= 1; dx++) {
                    for (int dz = -1; dz <= 1; dz++) {
                        long neighbor = pack(x + dx, z + dz);
                        if (neighbor != node) {
                            int cost = getLevel(neighbor) + 1;
                            if (computed > cost) {
                                computed = cost;
                            }
                        }
                    }
                }
                return computed;
            }
        }
    }

    // `b` is written after `this` left; `a` before. The other constructor writes after this(..).
    static final class Pair {
        final int a;
        final int b;
        int c;

        Pair(int a) {
            this(a, a + 1);
            c = a * 3;
        }

        Pair(int a, int b) {
            this.a = a;
            runAside(() -> System.out.println("pair thread"));
            register(this);
            this.b = b;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Grid grid = new Grid(6);
        grid.filler.join();
        long sum = 0;
        for (int round = 0; round < 200; round++) {
            for (int x = -5; x <= 5; x++) {
                for (int z = -5; z <= 5; z++) {
                    sum += grid.distances.computedLevel(pack(x, z), x, z);
                }
            }
        }
        System.out.println("sum " + sum);
        System.out.println("late " + grid.late + " origin " + grid.distances.origin);
        Pair p = new Pair(4);
        System.out.println("pair " + p.a + " " + p.b + " " + p.c);
        System.out.println("registered " + registered);
    }
}
