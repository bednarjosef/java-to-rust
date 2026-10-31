// Array fields that only their object's constructor makes and fills, and that the program then
// only indexes and measures, read by several threads: a shuffled permutation of a fixed length, a
// table its initializer makes and the constructor fills, one whose length the constructor is
// given, one that a constructor calling another makes, and a subclass reading its superclass's
// table. Beside them, an array field that a method writes after construction and one that
// leaves its object as a value, which stay shared arrays. A class's static table that a worker
// thread first initializes is read by the others as it was made. Indexes out of bounds throw as
// Java throws them.
public class FrozenArrayFields {
    static class Permutation {
        private final byte[] p;
        private final int[] squares = new int[8];
        private final long[] weights;

        Permutation(long seed, int n) {
            weights = new long[n];
            p = new byte[256];
            for (int i = 0; i < 256; i++) {
                p[i] = (byte) i;
            }
            long s = seed;
            for (int i = 0; i < 256; i++) {
                s = s * 6364136223846793005L + 1442695040888963407L;
                int j = i + (int) ((s >>> 33) % (256 - i));
                byte t = p[i];
                p[i] = p[j];
                p[j] = t;
            }
            for (int i = 0; i < squares.length; i++) {
                squares[i] = i * i;
            }
            squares[3] += 100;
            for (int i = 0; i < n; i++) {
                weights[i] = s >>> (40 + i);
            }
        }

        Permutation(long seed) {
            this(seed, 4);
        }

        int permute(int x) {
            return p[x & 255] & 255;
        }

        int square(int i) {
            return squares[i];
        }

        long weight(int i) {
            return weights[i % weights.length];
        }
    }

    static final class Scaled extends Permutation {
        final int factor;

        Scaled(long seed, int factor) {
            super(seed);
            this.factor = factor;
        }

        int scaled(int x) {
            return permute(x) * factor + square(x & 7);
        }
    }

    static final class Counter {
        final int[] counts = new int[4];
        final int[] given;

        Counter(int[] values) {
            given = new int[values.length];
            for (int i = 0; i < values.length; i++) {
                given[i] = values[i];
            }
        }

        synchronized void count(int i) {
            counts[i & 3]++;
        }

        int[] values() {
            return given;
        }
    }

    static final class Late {
        static final int[] TABLE = { 3, 1, 4, 1, 5, 9, 2, 6 };

        static {
            System.out.println("late table made");
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Permutation a = new Permutation(12345);
        Scaled b = new Scaled(678, 3);
        Counter counter = new Counter(new int[] { 7, 8, 9 });
        long[] sums = new long[4];
        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            int id = t;
            threads[t] = new Thread(() -> {
                long s = 0;
                for (int round = 0; round < 2000; round++) {
                    s += a.permute(round + id) + b.scaled(round * 7 + id) + a.weight(round);
                    s += Late.TABLE[round & 7];
                    counter.count(round);
                }
                sums[id] = s;
            });
            threads[t].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        for (int t = 0; t < sums.length; t++) {
            System.out.println("thread " + t + ": " + sums[t]);
        }
        StringBuilder first = new StringBuilder();
        for (int i = 0; i < 12; i++) {
            first.append(a.permute(i)).append(' ');
        }
        System.out.println(first);
        System.out.println(a.square(3) + " " + b.square(7) + " " + b.weight(5));
        System.out.println(counter.counts[0] + counter.counts[1] + counter.counts[2] + counter.counts[3]);
        int[] given = counter.values();
        given[0] = 70;
        System.out.println(counter.given[0] + " " + counter.given.length);
        try {
            System.out.println(a.square(8));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past the squares: " + e.getMessage());
        }
        try {
            System.out.println(a.weight(-1));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("before the weights: " + e.getMessage());
        }
    }
}
