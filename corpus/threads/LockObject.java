// Two counters, each guarded by its own lock object made with new Object().
public class LockObject {
    static final Object EVEN_LOCK = new Object();
    static final Object ODD_LOCK = new Object();
    static int evens;
    static int odds;

    static class Counter extends Thread {
        private final int from;

        Counter(int from) {
            this.from = from;
        }

        @Override
        public void run() {
            for (int i = from; i < from + 10000; i++) {
                if (i % 2 == 0) {
                    synchronized (EVEN_LOCK) {
                        evens++;
                    }
                } else {
                    synchronized (ODD_LOCK) {
                        odds++;
                    }
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Counter a = new Counter(0);
        Counter b = new Counter(1);
        Counter c = new Counter(7);
        a.start();
        b.start();
        c.start();
        a.join();
        b.join();
        c.join();
        System.out.println("evens " + evens + ", odds " + odds);
    }
}
