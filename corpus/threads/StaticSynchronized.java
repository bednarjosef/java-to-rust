// Static synchronized methods and a synchronized block on the class literal guard statics that
// several threads update.
public class StaticSynchronized {
    static int hits;
    static long total;

    static synchronized void hit(int amount) {
        hits++;
        total += amount;
    }

    static void hitTwice(int amount) {
        synchronized (StaticSynchronized.class) {
            hit(amount);
            hit(amount);
        }
    }

    static class Hitter extends Thread {
        private final int amount;

        Hitter(int amount) {
            this.amount = amount;
        }

        @Override
        public void run() {
            for (int i = 0; i < 10000; i++) {
                if (i % 2 == 0) {
                    hit(amount);
                } else {
                    hitTwice(amount);
                }
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Hitter[] hitters = {new Hitter(1), new Hitter(2), new Hitter(3)};
        for (Hitter h : hitters) {
            h.start();
        }
        for (Hitter h : hitters) {
            h.join();
        }
        System.out.println("hits " + hits + ", total " + total);
    }
}
