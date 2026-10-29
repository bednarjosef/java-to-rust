// Many threads that each allocate arrays of many sizes from 1 KB to 32 KB, then stay alive
// while the main thread builds a large structure, as the Minecraft server's worker threads do.
// Each thread keeps a span of free slots in every size it allocated in; what this measures is
// the memory those take (`scripts/benchmark.py` reports the peak).
public class ManyThreadsAllocating {
    static final int THREADS = 32;
    static final Object lock = new Object();
    static int allocated;
    static boolean released;

    public static void main(String[] args) throws InterruptedException {
        long[] sums = new long[THREADS];
        Thread[] threads = new Thread[THREADS];
        for (int t = 0; t < THREADS; t++) {
            final int id = t;
            threads[t] = new Thread(() -> {
                long sum = 0;
                for (int round = 0; round < 40; round++) {
                    for (int size = 1024; size <= 32768; size += 1024) {
                        Object[] refs = new Object[size / 8];
                        refs[round % refs.length] = refs;
                        byte[] bytes = new byte[size];
                        bytes[(round * 7) % size] = (byte) round;
                        sum += refs.length + bytes[(round * 7) % size];
                    }
                }
                sums[id] = sum;
                synchronized (lock) {
                    allocated++;
                    lock.notifyAll();
                    while (!released) {
                        try {
                            lock.wait();
                        } catch (InterruptedException e) {
                            throw new RuntimeException(e);
                        }
                    }
                }
            });
            threads[t].start();
        }
        synchronized (lock) {
            while (allocated < THREADS) {
                lock.wait();
            }
        }
        long[][] rows = new long[4000][];
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
