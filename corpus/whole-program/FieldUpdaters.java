// Field updaters held in static finals (`AtomicIntegerFieldUpdater.newUpdater(C.class, "f")` and
// the `long` and reference ones) are handles of their fields, as `findVarHandle` is: each access
// is the field's, and the updates are atomic between threads.
import java.util.concurrent.atomic.AtomicIntegerFieldUpdater;
import java.util.concurrent.atomic.AtomicLongFieldUpdater;
import java.util.concurrent.atomic.AtomicReferenceFieldUpdater;

public class FieldUpdaters {
    static final class Node {
        final String name;

        Node(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    private volatile int count;
    private volatile long total;
    private volatile Node head;

    private static final AtomicIntegerFieldUpdater<FieldUpdaters> COUNT =
            AtomicIntegerFieldUpdater.newUpdater(FieldUpdaters.class, "count");
    private static final AtomicLongFieldUpdater<FieldUpdaters> TOTAL =
            AtomicLongFieldUpdater.newUpdater(FieldUpdaters.class, "total");
    private static final AtomicReferenceFieldUpdater<FieldUpdaters, Node> HEAD =
            AtomicReferenceFieldUpdater.newUpdater(FieldUpdaters.class, Node.class, "head");

    void work() {
        for (int i = 0; i < 10_000; i++) {
            COUNT.getAndAdd(this, 1);
            TOTAL.getAndAdd(this, 3L);
        }
    }

    boolean push(Node n) {
        return HEAD.compareAndSet(this, null, n);
    }

    String show() {
        Node first = new Node("first");
        boolean pushed = push(first);
        boolean again = push(new Node("second"));
        Node old = HEAD.getAndSet(this, new Node("third"));
        HEAD.lazySet(this, HEAD.get(this));
        COUNT.set(this, 5);
        boolean swapped = COUNT.compareAndSet(this, 5, 6) && !COUNT.weakCompareAndSet(this, 5, 7);
        return pushed + " " + again + " " + old + " " + head + " " + swapped + " " + COUNT.get(this) + " "
                + COUNT.getAndSet(this, 0) + " " + TOTAL.get(this);
    }

    public static void main(String[] args) throws InterruptedException {
        FieldUpdaters u = new FieldUpdaters();
        System.out.println(u.show());
        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            threads[t] = new Thread(u::work);
            threads[t].start();
        }
        for (Thread thread : threads) {
            thread.join();
        }
        System.out.println(u.count + " " + u.total);
    }
}
