// The time of `synchronized` that threads contend for: two threads call the synchronized method of
// one counter 5 million times each, and then each locks the cells of a shared array of 100,000
// in turn, ten rounds, so that they meet on the same objects. Prints the kernel's time
// (`kernel-ms`).
public class ContendedSynchronized {
    static class Counter {
        long count;

        synchronized void add(long n) {
            count += n;
        }

        synchronized long get() {
            return count;
        }
    }

    static class Cell {
        long value;
    }

    public static void main(String[] args) throws InterruptedException {
        Counter shared = new Counter();
        Cell[] cells = new Cell[100_000];
        for (int i = 0; i < cells.length; i++) {
            cells[i] = new Cell();
        }
        long start = System.nanoTime();
        Thread[] threads = new Thread[2];
        for (int t = 0; t < threads.length; t++) {
            threads[t] = new Thread(() -> {
                for (int i = 0; i < 5_000_000; i++) {
                    shared.add(i & 3);
                }
                for (int round = 0; round < 10; round++) {
                    for (Cell c : cells) {
                        synchronized (c) {
                            c.value += 1;
                        }
                    }
                }
            });
            threads[t].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1_000_000);
        long sum = 0;
        for (Cell c : cells) {
            synchronized (c) {
                sum += c.value;
            }
        }
        System.out.println(shared.get() + " " + sum);
    }
}
