// Field updaters held in static finals (`AtomicIntegerFieldUpdater.newUpdater(C.class, "f")` and
// the `long` and reference ones) are handles of their fields, as `findVarHandle` is: each access
// is the field's, and the updates are atomic between threads. On an object other than `this`, a
// null one throws the updater's `ClassCastException`, which has no message.
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

    final class Inner {
        int bump() {
            return COUNT.incrementAndGet(FieldUpdaters.this);
        }
    }

    static String counts(FieldUpdaters other, long size) {
        int a = COUNT.getAndIncrement(other);
        int b = COUNT.incrementAndGet(other);
        int c = COUNT.getAndDecrement(other);
        int d = COUNT.decrementAndGet(other);
        int step = 3;
        int e = COUNT.addAndGet(other, -step);
        long f = TOTAL.addAndGet(other, size);
        long g = TOTAL.addAndGet(other, -size);
        long h = TOTAL.getAndIncrement(other);
        long i = TOTAL.decrementAndGet(other);
        return a + " " + b + " " + c + " " + d + " " + e + " " + f + " " + g + " " + h + " " + i + " "
                + other.new Inner().bump();
    }

    static void refused(FieldUpdaters none) {
        try {
            COUNT.incrementAndGet(none);
        } catch (ClassCastException e) {
            System.out.println("caught " + e + " " + e.getMessage());
        }
        try {
            HEAD.compareAndSet(none, null, null);
        } catch (RuntimeException e) {
            System.out.println("caught " + e);
        }
        try {
            System.out.println(TOTAL.get(none));
        } catch (ClassCastException e) {
            System.out.println("caught " + e);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        FieldUpdaters u = new FieldUpdaters();
        System.out.println(u.show());
        System.out.println(counts(new FieldUpdaters(), args.length + 2L));
        refused(args.length > 5 ? u : null);
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
