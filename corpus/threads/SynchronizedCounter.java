// A counter that several threads increment through synchronized methods: no increment is lost.
public class SynchronizedCounter {
    static class Counter {
        private int count;

        synchronized void increment() {
            count++;
        }

        synchronized int get() {
            return count;
        }
    }

    static class Incrementer extends Thread {
        private final Counter counter;
        private final int times;

        Incrementer(Counter counter, int times) {
            this.counter = counter;
            this.times = times;
        }

        @Override
        public void run() {
            for (int i = 0; i < times; i++) {
                counter.increment();
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Counter counter = new Counter();
        Incrementer[] workers = new Incrementer[4];
        for (int i = 0; i < workers.length; i++) {
            workers[i] = new Incrementer(counter, 20000);
            workers[i].start();
        }
        for (Incrementer w : workers) {
            w.join();
        }
        System.out.println("count " + counter.get());
    }
}
