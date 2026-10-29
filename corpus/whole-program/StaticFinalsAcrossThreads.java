import java.util.ArrayList;
import java.util.List;

// Static finals of several kinds, set once by their class's static initialization and read by
// many threads at once: an array of objects that is indexed and whose elements are called, an
// array of ints, an object whose fields and methods are used, a string, a static that stays null,
// a list that the threads change through its methods, and a lock object. Each thread may be the
// first to use a class, so the others wait for its initialization and then see every value.
public class StaticFinalsAcrossThreads {
    static final class Gradient {
        final int x;
        final int y;

        Gradient(int x, int y) {
            this.x = x;
            this.y = y;
        }

        int dot(int a, int b) {
            return x * a + y * b;
        }
    }

    static final class Table {
        private static final Gradient[] GRADIENTS;
        private static final int[] WEIGHTS = {3, 1, 4, 1, 5, 9, 2, 6};
        static final Gradient ORIGIN = new Gradient(0, 0);
        static final String NAME;
        static final Object NOTHING = null;
        static final List<Integer> SEEN = new ArrayList<>();
        static final Object LOCK = new Object();

        static {
            Gradient[] made = new Gradient[8];
            for (int i = 0; i < made.length; i++) {
                made[i] = new Gradient(i % 3 - 1, i / 3 - 1);
            }
            GRADIENTS = made;
            StringBuilder name = new StringBuilder("table");
            name.append('-').append(GRADIENTS.length);
            NAME = name.toString();
        }

        static int at(int hash, int a, int b) {
            return GRADIENTS[hash & 7].dot(a, b) * WEIGHTS[hash & 7];
        }

        static int sumOfWeights() {
            int sum = 0;
            for (int w : WEIGHTS) {
                sum += w;
            }
            return sum;
        }
    }

    // Initialized lazily: its statics have values of their own from the start.
    static final class Constants {
        static final int[] PRIMES = {2, 3, 5, 7, 11};
        static final String GREETING = "hello " + PRIMES.length;
    }

    public static void main(String[] args) throws InterruptedException {
        int threads = 6;
        long[] sums = new long[threads];
        List<Thread> started = new ArrayList<>();
        for (int t = 0; t < threads; t++) {
            int id = t;
            Thread thread = new Thread(() -> {
                long sum = 0;
                for (int i = 0; i < 20000; i++) {
                    sum += Table.at(i * 31 + id, i % 5, id);
                    sum += Table.ORIGIN.x + Table.ORIGIN.dot(i, id);
                    sum += Table.NAME.length() + Table.NAME.charAt(i % Table.NAME.length());
                    sum += Constants.PRIMES[i % Constants.PRIMES.length];
                    if (Table.NOTHING != null) {
                        sum = -1;
                    }
                }
                sum += Constants.GREETING.length();
                synchronized (Table.LOCK) {
                    Table.SEEN.add(id);
                }
                sums[id] = sum;
            });
            started.add(thread);
        }
        for (Thread thread : started) {
            thread.start();
        }
        for (Thread thread : started) {
            thread.join();
        }
        for (int t = 0; t < threads; t++) {
            System.out.println("thread " + t + ": " + sums[t]);
        }
        List<Integer> seen = new ArrayList<>(Table.SEEN);
        seen.sort(null);
        System.out.println(seen);
        System.out.println(Table.NAME + " " + Table.sumOfWeights() + " " + Table.NOTHING);
        System.out.println(Constants.GREETING);
    }
}
