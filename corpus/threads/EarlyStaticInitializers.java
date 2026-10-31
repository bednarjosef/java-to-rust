// Static initializers that nothing can observe, which may run before main does anything (a table
// of gradient objects, read from worker threads), and those that must run at the class's first
// use: one that prints, one that reads another class's static that main changes first, and one
// that throws.
public class EarlyStaticInitializers {
    static int counter;

    static final class Gradient {
        final double x, y, z;

        Gradient(double x, double y, double z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }

        double dot(double a, double b, double c) {
            return x * a + y * b + z * c;
        }
    }

    // Only allocates objects whose constructors assign fields, and arrays of literals.
    static final class Noise {
        static final Gradient[] GRADIENT = {
            new Gradient(1, 1, 0), new Gradient(-1, 1, 0), new Gradient(1, -1, 0), new Gradient(-1, -1, 0),
            new Gradient(1, 0, 1), new Gradient(-1, 0, 1), new Gradient(1, 0, -1), new Gradient(-1, 0, -1),
        };
        static final int[] PERMUTATION = {3, 1, 4, 1, 5, 9, 2, 6};
        static final Gradient UP = new Gradient(0, 1, 0);
        static final String NAME = "noise";

        static double grad(int h, double x, double y, double z) {
            return GRADIENT[PERMUTATION[h & 7] & 7].dot(x, y, z) + UP.dot(x, y, z);
        }
    }

    static final class Prints {
        static final Gradient[] TABLE = {new Gradient(0, 0, 1)};

        static {
            System.out.println("Prints initialized");
        }
    }

    static final class ReadsOther {
        static final int[] SNAPSHOT = {counter, counter * 2};
        static final Gradient AT = new Gradient(counter, 0, 0);
    }

    static final class Throws {
        static final int[] TABLE = {10};

        static {
            if (TABLE.length == 1) {
                throw new IllegalStateException("no table");
            }
        }
    }

    static double sample(int seed, int rounds) {
        double sum = 0;
        for (int i = 0; i < rounds; i++) {
            int h = seed * 17 + i;
            sum += Noise.grad(h, i * 0.5, 1.0 - i * 0.25, 0.125) * Noise.grad(h >> 3, 1.0, 0.5, -1.0);
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        System.out.println("start");
        counter = 21;
        double[] sums = new double[3];
        Thread[] threads = new Thread[3];
        for (int t = 0; t < threads.length; t++) {
            int k = t;
            threads[t] = new Thread(() -> sums[k] = sample(k, 10000 + k));
            threads[t].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        for (double s : sums) {
            System.out.println((long) s);
        }
        System.out.println(Noise.NAME + " " + Noise.GRADIENT.length + " " + Noise.UP.y);
        System.out.println("before Prints");
        System.out.println(Prints.TABLE[0].z);
        System.out.println(ReadsOther.SNAPSHOT[0] + " " + ReadsOther.SNAPSHOT[1] + " " + ReadsOther.AT.x);
        try {
            System.out.println(Throws.TABLE[0]);
        } catch (ExceptionInInitializerError e) {
            System.out.println("Throws failed: " + e.getCause().getMessage());
        }
    }
}
