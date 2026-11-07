// Atomics that one thread uses and atomics that threads share. A local AtomicInteger, AtomicLong
// or AtomicReference that never leaves the method that makes it, also when a lambda that the
// method passes down captures it, is used only by the thread that runs the method, so its reads,
// writes and compare-and-sets are plain ones (docs/multi-threading.md, "Objects that one thread
// uses"). The atomics that the workers reach through statics keep their atomic updates, and four
// threads lose none of them.
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.IntConsumer;

public class ConfinedAtomics {
    static final AtomicInteger sharedCount = new AtomicInteger();
    static final AtomicLong sharedTotal = new AtomicLong();
    static final AtomicReference<String> firstDone = new AtomicReference<>();

    static void each(int n, IntConsumer action) {
        for (int i = 0; i < n; i++) {
            action.accept(i);
        }
    }

    // The last index whose value is over the limit, which the lambda records.
    static int lastOver(int[] values, int limit) {
        AtomicInteger last = new AtomicInteger(-1);
        each(values.length, i -> {
            if (values[i] > limit) {
                last.set(i);
            }
        });
        return last.get();
    }

    // The longest word, swapped in by compare-and-set.
    static String longest(String[] words) {
        AtomicReference<String> best = new AtomicReference<>("");
        each(words.length, i -> {
            String current = best.get();
            if (words[i].length() > current.length() && !best.compareAndSet(current, words[i])) {
                System.out.println("lost a swap");
            }
        });
        return best.get();
    }

    // Steps a counter by compare-and-set, get-and-add and get-and-set.
    static String steps(int n) {
        AtomicInteger count = new AtomicInteger();
        AtomicLong sum = new AtomicLong(100);
        for (int i = 0; i < n; i++) {
            boolean swapped = count.compareAndSet(i, i + 1);
            boolean missed = count.compareAndSet(i, -1);
            sum.getAndAdd(i);
            if (!swapped || missed) {
                return "wrong at " + i;
            }
        }
        int old = count.getAndSet(7);
        return old + " " + count.get() + " " + count.incrementAndGet() + " " + sum.get() + " "
                + sum.compareAndExchange(sum.get(), 5) + " " + sum.get();
    }

    static class Worker extends Thread {
        final int id;

        Worker(int id) {
            this.id = id;
        }

        @Override
        public void run() {
            // Made on this thread and used by it alone.
            AtomicInteger mine = new AtomicInteger();
            for (int i = 0; i < 10000; i++) {
                mine.set(mine.get() + 1);
                sharedCount.incrementAndGet();
                sharedTotal.addAndGet(i);
            }
            firstDone.compareAndSet(null, "done");
            if (mine.get() != 10000) {
                System.out.println("worker " + id + " counted " + mine.get());
            }
        }
    }

    public static void main(String[] args) throws InterruptedException {
        int[] values = {3, 9, 1, 12, 4, 15, 2};
        System.out.println(lastOver(values, 8) + " " + lastOver(values, 20));
        System.out.println(longest(new String[] {"ash", "maple", "oak", "sycamore", "elm", "hornbeam"}));
        System.out.println(steps(10));
        Worker[] workers = new Worker[4];
        for (int i = 0; i < workers.length; i++) {
            workers[i] = new Worker(i);
            workers[i].start();
        }
        for (Worker w : workers) {
            w.join();
        }
        System.out.println(sharedCount.get() + " " + sharedTotal.get() + " " + firstDone.get());
    }
}
