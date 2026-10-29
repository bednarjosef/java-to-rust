// Many small objects in a program that waits and notifies on one lock object, an `Object`: 3
// million pairs of `long`s, all alive at once, which are `Object`s somewhere too, but which no
// monitor site can reach, so they keep no monitor. What it measures is the memory they take
// (`scripts/benchmark.py` reports the peak).
public class UnlockedObjects {
    static class Pair {
        long a;
        long b;

        Pair(long a, long b) {
            this.a = a;
            this.b = b;
        }
    }

    static final Object lock = new Object();
    static boolean ready;

    public static void main(String[] args) throws InterruptedException {
        Thread starter = new Thread(() -> {
            synchronized (lock) {
                ready = true;
                lock.notifyAll();
            }
        });
        starter.start();
        synchronized (lock) {
            while (!ready) {
                lock.wait();
            }
        }
        starter.join();
        Pair[] pairs = new Pair[3_000_000];
        for (int i = 0; i < pairs.length; i++) {
            pairs[i] = new Pair(i, 2L * i);
        }
        for (int round = 0; round < 3; round++) {
            for (Pair p : pairs) {
                p.a = p.a * 3 + p.b;
            }
        }
        Object[] some = {pairs[7], pairs[11]};
        long sum = ((Pair) some[0]).a - ((Pair) some[1]).b;
        for (Pair p : pairs) {
            sum += p.a + p.b;
        }
        System.out.println(sum);
    }
}
