// Classes of a hierarchy first initialized by worker threads, which then use them in a loop: a
// subclass's initialization runs its superclass's first, each once, whichever thread gets there
// first, and the checks a thread skips once it has passed one are those that would pass.
public class ClassInitChecksAcrossThreads {
    static class Base {
        static final int[] STEPS;
        static int scale;

        static {
            STEPS = new int[] { 3, 5, 7 };
            scale = 2;
            System.out.println("init Base");
        }

        final int value;

        Base(int value) {
            this.value = value;
        }

        static int step(int i) {
            return STEPS[i % STEPS.length] * scale;
        }
    }

    static class Leaf extends Base {
        static final int LIMIT;

        static {
            LIMIT = 1_000_003;
            System.out.println("init Leaf");
        }

        Leaf(int value) {
            super(value % LIMIT);
        }

        Leaf grow(int i) {
            return new Leaf(value * 3 + step(i));
        }

        static int fold(int a, int b) {
            return (a * 31 + b) % LIMIT;
        }
    }

    static int work(int seed, int n) {
        Leaf leaf = new Leaf(seed);
        int total = 0;
        for (int i = 0; i < n; i++) {
            leaf = leaf.grow(i);
            total = Leaf.fold(total, leaf.value);
            total = Leaf.fold(total, Base.step(i) + Base.scale);
        }
        return total;
    }

    public static void main(String[] args) throws InterruptedException {
        int[] results = new int[4];
        Thread[] threads = new Thread[results.length];
        for (int t = 0; t < threads.length; t++) {
            final int k = t;
            threads[t] = new Thread(() -> results[k] = work(k + 1, 10_000));
            threads[t].start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        for (int r : results) {
            System.out.println(r);
        }
        System.out.println(work(9, 100));
    }
}
