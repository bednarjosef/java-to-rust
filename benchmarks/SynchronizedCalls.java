// The time of `synchronized` in a program whose threads share the objects they lock, as the
// collections of the JDK and the Minecraft server's do: a counter whose synchronized method is
// called 30 million times, and a million objects of which each is locked in turn, ten rounds, as
// `ConcurrentHashMap.putVal` locks the first node of a bin. A second thread runs beside the main
// one throughout, so every monitor is a real lock. Prints the kernel's time (`kernel-ms`).
public class SynchronizedCalls {
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

        Cell(long value) {
            this.value = value;
        }
    }

    static final Object lock = new Object();
    static boolean done;

    public static void main(String[] args) throws InterruptedException {
        Counter shared = new Counter();
        Thread other = new Thread(() -> {
            synchronized (lock) {
                while (!done) {
                    try {
                        lock.wait();
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
            shared.add(1);
        });
        other.start();
        Cell[] cells = new Cell[1_000_000];
        for (int i = 0; i < cells.length; i++) {
            cells[i] = new Cell(i);
        }
        long start = System.nanoTime();
        for (int i = 0; i < 30_000_000; i++) {
            shared.add(i & 7);
        }
        long sum = 0;
        for (int round = 0; round < 10; round++) {
            for (Cell c : cells) {
                synchronized (c) {
                    c.value += round;
                    sum += c.value;
                }
            }
        }
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1_000_000);
        synchronized (lock) {
            done = true;
            lock.notifyAll();
        }
        other.join();
        System.out.println(shared.get() + " " + sum);
    }
}
