// Objects that only the fields of other objects reach, while threads make enough garbage for
// the collector to run many times: linked lists whose nodes are found only through `next`,
// strings compared by identity that only a field holds, arrays held in fields, a `volatile`
// field that one thread publishes and another polls, and a field swapped by compare-and-set
// through an `AtomicReferenceFieldUpdater`. Every object must survive while reachable and keep
// its contents, whatever form the translation gives the fields' references.
import java.util.concurrent.atomic.AtomicReferenceFieldUpdater;

public class ReferencesOnlyInFields {
    static class Node {
        final int value;
        Node next;
        String label;
        int[] digits;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
            this.label = ("node " + value).intern();
            this.digits = new int[] {value % 10, value / 10 % 10, value / 100 % 10};
        }
    }

    static class Holder {
        volatile Node published;
        volatile Node swapped;

        static final AtomicReferenceFieldUpdater<Holder, Node> SWAPPED =
            AtomicReferenceFieldUpdater.newUpdater(Holder.class, Node.class, "swapped");
    }

    static Node chain(int n) {
        Node head = null;
        for (int i = n - 1; i >= 0; i--) {
            head = new Node(i, head);
        }
        return head;
    }

    static long garbage(int rounds) {
        long sum = 0;
        for (int i = 0; i < rounds; i++) {
            Node[] nodes = new Node[16];
            for (int j = 0; j < nodes.length; j++) {
                nodes[j] = new Node(j, j > 0 ? nodes[j - 1] : null);
            }
            sum += nodes[15].next.value + nodes[i & 15].label.length();
        }
        return sum;
    }

    static String check(Node head, int n) {
        int count = 0;
        for (Node node = head; node != null; node = node.next) {
            if (node.value != count || node.label != ("node " + count).intern()
                || node.digits[0] != count % 10 || node.digits[2] != count / 100 % 10) {
                return "wrong node at " + count;
            }
            count++;
        }
        return count == n ? "ok" : "wrong length " + count;
    }

    public static void main(String[] args) throws InterruptedException {
        Holder holder = new Holder();
        Node[] kept = new Node[4];
        long[] sums = new long[4];
        Thread[] workers = new Thread[4];
        for (int t = 0; t < workers.length; t++) {
            final int index = t;
            workers[t] = new Thread(() -> {
                kept[index] = chain(2000 + index);
                sums[index] = garbage(30000);
                Node mine = new Node(1000 + index, null);
                Holder shared = holder;
                while (true) {
                    Node old = shared.swapped;
                    mine.next = old;
                    if (Holder.SWAPPED.compareAndSet(shared, old, mine)) {
                        break;
                    }
                }
            });
            workers[t].start();
        }
        Thread publisher = new Thread(() -> {
            garbage(10000);
            holder.published = chain(500);
        });
        publisher.start();
        while (holder.published == null) {
            Thread.onSpinWait();
        }
        for (Thread worker : workers) {
            worker.join();
        }
        publisher.join();
        garbage(20000);
        for (int t = 0; t < kept.length; t++) {
            System.out.println("chain " + t + ": " + check(kept[t], 2000 + t) + ", garbage sum " + sums[t]);
        }
        System.out.println("published: " + check(holder.published, 500));
        int swaps = 0;
        int total = 0;
        for (Node node = holder.swapped; node != null; node = node.next) {
            swaps++;
            total += node.value;
        }
        System.out.println("swapped: " + swaps + " nodes, values sum to " + total);
    }
}
