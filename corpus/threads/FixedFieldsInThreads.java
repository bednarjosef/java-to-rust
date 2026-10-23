// Fields that only constructors write, read by several threads: a worker whose fields are fixed,
// and a class whose constructor starts a thread, whose fields stay under the lock.
public class FixedFieldsInThreads {
    static final class Worker implements Runnable {
        private final String name;
        private final Counts counts;
        private int done;

        Worker(String name, Counts counts) {
            this.name = name;
            this.counts = counts;
        }

        public void run() {
            for (int i = 0; i < 1000; i++) {
                counts.add();
            }
            synchronized (this) {
                done++;
            }
        }

        synchronized int done() {
            return done;
        }
    }

    static final class Counts {
        private int total;

        synchronized void add() {
            total++;
        }

        synchronized int total() {
            return total;
        }
    }

    static final class Starter {
        private final String label;
        private final Thread thread;

        Starter(String label, Runnable body) throws InterruptedException {
            this.label = label;
            thread = new Thread(body);
            thread.start();
            thread.join();
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Counts counts = new Counts();
        Worker worker = new Worker("w", counts);
        Thread[] threads = new Thread[4];
        for (int i = 0; i < threads.length; i++) {
            threads[i] = new Thread(worker);
            threads[i].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        System.out.println(worker.name + " " + worker.done() + " " + counts.total());
        Starter s = new Starter("s", worker);
        System.out.println(s.label + " " + worker.done() + " " + counts.total() + " " + s.thread.isAlive());
    }
}
