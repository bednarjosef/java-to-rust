// A class whose constructor starts a thread but never lets `this` leave keeps its fields fixed:
// no other thread can hold the object until the constructor is done, so they are read as plain
// loads. A class whose constructor hands `this` to its thread reads them as shared fields.
public class FixedFieldsOfThreadStartingConstructors {
    static final class Pool {
        final int[] slots;
        final Thread worker;

        Pool(int n) {
            slots = new int[n];
            worker = new Thread(() -> System.out.println("worker ran"));
            worker.start();
        }

        int sum() {
            int s = 0;
            for (int i = 0; i < slots.length; i++) {
                s += slots[i];
            }
            return s;
        }
    }

    static final class Reporter {
        final int[] counts;
        final Thread thread;
        volatile boolean ready;

        Reporter(int n) {
            counts = new int[n];
            thread = new Thread(this::report);
            for (int i = 0; i < n; i++) {
                counts[i] = i * i;
            }
            ready = true;
            thread.start();
        }

        void report() {
            while (!ready) {
                Thread.onSpinWait();
            }
            int s = 0;
            for (int c : counts) {
                s += c;
            }
            System.out.println("reported " + s);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Pool pool = new Pool(8);
        pool.worker.join();
        for (int i = 0; i < pool.slots.length; i++) {
            pool.slots[i] = i + 1;
        }
        System.out.println("pool " + pool.sum());
        Reporter reporter = new Reporter(5);
        reporter.thread.join();
        System.out.println("counts " + reporter.counts.length);
    }
}
