// Threads that each make one deep recursion and then wait, while the main thread builds a large
// structure, as the Minecraft server's world generation threads go deep and then idle. A thread
// keeps the pages of stack it touched unless they are given back; what this measures is the memory
// they take (`scripts/benchmark.py` reports the peak).
public class DeepThreadStacks {
    static final int THREADS = 16;
    static final int DEPTH = 6000;
    static final Object lock = new Object();
    static int deep;
    static boolean released;

    // Each frame keeps eight values across the call, as a recursive descent over a structure does.
    static long descend(int depth, long a, long b, long c) {
        if (depth == 0) {
            return a ^ b ^ c;
        }
        long d = a * 31 + depth;
        long e = b * 17 + d;
        long f = c * 13 + e;
        long g = d ^ (e >>> 3);
        long h = e ^ (f << 5);
        long i = f + g * 7;
        long j = g - h * 3;
        long k = h ^ i ^ j;
        long below = descend(depth - 1, d, e, f);
        return Long.rotateLeft(below, (int) (k & 7)) ^ (d + e + f + g + h + i + j + k);
    }

    public static void main(String[] args) throws InterruptedException {
        long[] sums = new long[THREADS];
        Thread[] threads = new Thread[THREADS];
        for (int t = 0; t < THREADS; t++) {
            final int id = t;
            threads[t] = new Thread(null, () -> {
                sums[id] = descend(DEPTH, id, id + 1, id + 2);
                synchronized (lock) {
                    deep++;
                    lock.notifyAll();
                    while (!released) {
                        try {
                            lock.wait();
                        } catch (InterruptedException e) {
                            throw new RuntimeException(e);
                        }
                    }
                }
            }, "deep-" + t, 16 << 20);
            threads[t].start();
        }
        synchronized (lock) {
            while (deep < THREADS) {
                lock.wait();
            }
        }
        long[][] rows = new long[12000][];
        long total = 0;
        for (int round = 0; round < 6; round++) {
            for (int i = 0; i < rows.length; i++) {
                rows[i] = new long[1000 + (i + round) % 7];
                rows[i][i % 1000] = i + round;
                total += rows[i][i % 1000];
            }
        }
        synchronized (lock) {
            released = true;
            lock.notifyAll();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        for (long s : sums) {
            total += s;
        }
        System.out.println(total);
    }
}
