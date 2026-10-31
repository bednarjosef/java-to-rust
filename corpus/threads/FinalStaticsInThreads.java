// Static final tables of classes that a started thread initializes, read from static helpers in
// loops on several threads: arrays of primitives, of objects and of possibly-null objects, a
// string, an object, and a class whose initializer reads its own statics before and after
// assigning them.
public class FinalStaticsInThreads {
    static final class Vec3 {
        final double x, y, z;

        Vec3(double x, double y, double z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        double dot(double a, double b, double c) {
            return x * a + y * b + z * c;
        }
    }

    static final class Marker {
        final int id;

        Marker(int id) {
            this.id = id;
        }
    }

    static final class Tables {
        static int initializations;
        static final int[] PRIMES = {2, 3, 5, 7, 11, 13, 17, 19};
        static final Vec3[] DIRECTIONS = {
            new Vec3(1, 0, 0), new Vec3(-1, 0, 0), new Vec3(0, 1, 0), new Vec3(0, -1, 0),
            new Vec3(0, 0, 1), new Vec3(0, 0, -1), new Vec3(1, 1, 0), new Vec3(0, 1, 1),
        };
        static final Marker[] SPARSE = new Marker[4];
        static final double[] WEIGHTS;
        static final String NAME;
        static final Vec3 ORIGIN = new Vec3(0.5, 0.25, 0.125);

        static {
            initializations++;
            WEIGHTS = new double[PRIMES.length];
            for (int i = 0; i < PRIMES.length; i++) {
                WEIGHTS[i] = 1.0 / PRIMES[i];
            }
            SPARSE[1] = new Marker(1);
            SPARSE[3] = new Marker(PRIMES[3]);
            NAME = "tables of " + PRIMES.length;
        }

        static int prime(int i) {
            return PRIMES[i & 7];
        }

        static double weight(int i) {
            return WEIGHTS[i & 7];
        }

        static double along(int i, double a, double b, double c) {
            return DIRECTIONS[i & 7].dot(a, b, c) + ORIGIN.dot(a, b, c);
        }

        static int present() {
            int n = 0;
            for (Marker m : SPARSE) {
                if (m != null) {
                    n += m.id;
                }
            }
            return n;
        }
    }

    // Reads its tables before and after they are assigned, from its own initializer.
    static final class Early {
        static final int[] SEEDS;
        static final String LABEL;
        static final boolean SEEDS_WERE_NULL;
        static final boolean LABEL_WAS_NULL;

        static {
            SEEDS_WERE_NULL = seeds() == null;
            LABEL_WAS_NULL = label() == null;
            SEEDS = new int[] {4, 8, 15, 16, 23, 42};
            LABEL = "early " + seeds().length;
        }

        static int[] seeds() {
            return SEEDS;
        }

        static String label() {
            return LABEL;
        }
    }

    // No observable initializer: its tables are built on first read.
    static final class Pure {
        static final long[] POWERS = {1, 10, 100, 1000, 10000};
        static final String WORD = "pure";
    }

    static double work(int seed, int rounds) {
        double sum = 0;
        for (int i = 0; i < rounds; i++) {
            int h = seed * 31 + i;
            sum += Tables.prime(h) * Tables.weight(h >> 3);
            sum += Tables.along(h, 0.5, -0.25, 1.0) + Tables.along(h >> 2, 1.0, 1.0, -1.0);
            sum += Pure.POWERS[i % 5] % 7;
            if (sum > 1e12) {
                return sum;
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        double[] sums = new double[4];
        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            int k = t;
            threads[t] = new Thread(() -> sums[k] = work(k, 20000 + k));
        }
        for (Thread t : threads) {
            t.start();
        }
        for (Thread t : threads) {
            t.join();
        }
        for (double s : sums) {
            System.out.println((long) (s * 1000));
        }
        System.out.println(Tables.NAME + " " + Tables.initializations + " " + Tables.present());
        Thread early = new Thread(() -> System.out.println(Early.label() + " " + Early.SEEDS_WERE_NULL
                + " " + Early.LABEL_WAS_NULL + " " + Early.seeds()[5]));
        early.start();
        early.join();
        System.out.println(Pure.WORD + " " + Pure.POWERS[4] + " " + work(9, 100));
    }
}
