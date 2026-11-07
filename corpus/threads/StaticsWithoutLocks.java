// Statics of the process that are assigned again after their class's initialization, which
// threads read and write with no lock: a message published through a volatile flag, a list a
// static's value is replaced along, a string appended to in turns, racing writes of a box, and
// the store-then-load of two volatile statics, which Java's sequential consistency for volatiles
// keeps from both reading the other's old value.
public class StaticsWithoutLocks {
    static String message;
    static volatile boolean ready;
    static Node head;
    static String log = "start";
    static Integer last;
    static volatile Object left;
    static volatile Object right;
    static volatile int turns;

    static class Node {
        final int value;
        final Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Thread reader = new Thread(() -> {
            while (!ready) {
                Thread.onSpinWait();
            }
            System.out.println("read " + message);
        });
        reader.start();
        message = "published";
        ready = true;
        reader.join();

        for (int i = 0; i < 4; i++) {
            int value = i;
            Thread pusher = new Thread(() -> {
                head = new Node(value, head);
                log += " " + value;
                turns++;
            });
            pusher.start();
            pusher.join();
        }
        StringBuilder list = new StringBuilder();
        for (Node n = head; n != null; n = n.next) {
            list.append(n.value).append(' ');
        }
        System.out.println("list " + list.toString().trim());
        System.out.println("log " + log + ", turns " + turns);

        Thread[] racers = new Thread[4];
        for (int i = 0; i < racers.length; i++) {
            int base = i * 1000;
            Thread racer = new Thread(() -> {
                for (int k = 0; k < 1000; k++) {
                    last = base + k;
                }
            });
            racers[i] = racer;
            racer.start();
        }
        for (Thread racer : racers) {
            racer.join();
        }
        System.out.println("last is one written last by a racer: " + (last % 1000 == 999));

        int bothOld = 0;
        Object token = new Object();
        for (int round = 0; round < 300; round++) {
            left = null;
            right = null;
            Object[] seen = new Object[2];
            Thread a = new Thread(() -> {
                left = token;
                seen[0] = right;
            });
            Thread b = new Thread(() -> {
                right = token;
                seen[1] = left;
            });
            a.start();
            b.start();
            a.join();
            b.join();
            if (seen[0] == null && seen[1] == null) {
                bothOld++;
            }
        }
        System.out.println("rounds where both read the old value: " + bothOld);
        System.out.println("head is null: " + (head == null));
    }
}
