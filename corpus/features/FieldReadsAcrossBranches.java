// A field read again after a test, an early return, or a store of its own is read once only where
// nothing between can change it: a store through another reference to the same object, a call,
// a volatile read, or the initialization of another class must each see the new value; a field
// found null throws with Java's message. The objects are used on a worker thread too, so their
// fields are ones that threads may share.
public class FieldReadsAcrossBranches {
    static final class Node {
        int value;
        int weight;
        Node next;
        String name;

        Node(int value, int weight, String name) {
            this.value = value;
            this.weight = weight;
            this.name = name;
        }

        int weigh(int scale) {
            return this.weight * scale + this.value;
        }
    }

    static final class Holder {
        static Node held;
    }

    static final class Late {
        static final int SEEN;

        static {
            Holder.held.value = 99;
            SEEN = 1;
        }
    }

    static final class Mover {
        Node vehicle;
        Node target;
        int seed = 7;
        int moved;
        volatile boolean ready;

        int turn(int by) {
            if (this.vehicle != null) {
                this.moved += this.vehicle.weigh(by);
            }
            return this.moved;
        }

        int follow() {
            if (this.target == null) {
                return -1;
            }
            int sum = this.target.value + this.target.weight;
            if (this.target.next == null) {
                return sum;
            }
            return sum + this.target.next.value;
        }

        int pick() {
            return this.target == null ? 0 : this.target.value;
        }

        int roll() {
            this.seed = this.seed * 3 + 1;
            int a = this.seed >> 1;
            this.seed += a;
            int b = this.seed;
            ++this.seed;
            return a + b + this.seed;
        }

        // A store through another reference to the same object.
        int aliased(Mover other) {
            int before = this.seed;
            other.seed = before + 5;
            return before + this.seed;
        }

        // A call that changes the field.
        int afterCall() {
            int first = this.vehicle.value;
            this.vehicle.value = first + 1;
            touch(this.vehicle);
            return first + this.vehicle.value;
        }

        // A volatile read between two reads.
        int afterVolatile(Node n) {
            int a = n.value;
            boolean r = this.ready;
            int b = n.value;
            return r ? a * 100 + b : b;
        }

        // The first use of a class whose initializer stores to the field.
        int afterInitializer(Node n) {
            int a = n.value;
            int seen = Late.SEEN;
            return a * 1000 + n.value + seen;
        }

        // A branch that sets the field: the code after the `if` reads it again.
        int branch(boolean drop) {
            int a = this.target == null ? 0 : this.target.value;
            if (drop) {
                this.target = null;
            }
            return a + (this.target == null ? -10 : this.target.value);
        }

        // A value started in a branch is not taken after it.
        String names(boolean first) {
            String s;
            if (first) {
                s = this.vehicle.name + "/" + this.vehicle.name;
            } else {
                s = "none";
            }
            return s + ":" + (this.vehicle == null ? "-" : this.vehicle.name);
        }
    }

    static void touch(Node n) {
        n.value *= 2;
    }

    static int elements(Node[] nodes, int i) {
        Node a = nodes[i];
        nodes[i] = nodes[(i + 1) % nodes.length];
        return a.value + nodes[i].value;
    }

    static String npe(Mover m) {
        try {
            return "value " + m.follow() + " " + m.target.next.value;
        } catch (NullPointerException e) {
            return e.getMessage();
        }
    }

    static String npeTwice(Mover m) {
        try {
            int a = m.vehicle.value;
            return "value " + (a + m.vehicle.weight);
        } catch (NullPointerException e) {
            return e.getMessage();
        }
    }

    static void run(String label) {
        Node[] nodes = new Node[4];
        for (int i = 0; i < nodes.length; i++) {
            nodes[i] = new Node(i * 3 + 1, i + 2, "n" + i);
        }
        nodes[1].next = nodes[2];
        Mover m = new Mover();
        StringBuilder out = new StringBuilder(label);
        out.append(' ').append(m.turn(3)).append(' ').append(m.follow()).append(' ').append(m.pick());
        m.vehicle = nodes[0];
        m.target = nodes[1];
        out.append(' ').append(m.turn(3)).append(' ').append(m.follow()).append(' ').append(m.pick());
        m.target = nodes[2];
        out.append(' ').append(m.follow()).append(' ').append(m.roll()).append(' ').append(m.roll());
        out.append(' ').append(m.aliased(m)).append(' ').append(m.aliased(new Mover()));
        out.append(' ').append(m.afterCall()).append(' ').append(m.afterVolatile(nodes[3]));
        m.ready = true;
        out.append(' ').append(m.afterVolatile(nodes[3]));
        out.append(' ').append(m.branch(false)).append(' ').append(m.branch(true));
        out.append(' ').append(m.names(true)).append(' ').append(m.names(false));
        out.append(' ').append(elements(nodes, 1)).append(' ').append(elements(nodes, 3));
        System.out.println(out);
        Mover empty = new Mover();
        empty.target = new Node(1, 1, "lone");
        System.out.println(npe(empty));
        System.out.println(npeTwice(empty));
        if (Holder.held == null) {
            Holder.held = nodes[2];
            System.out.println("initializer " + m.afterInitializer(nodes[2]));
        }
    }

    public static void main(String[] args) throws InterruptedException {
        run("main");
        Thread worker = new Thread(() -> run("worker"));
        worker.start();
        worker.join();
    }
}
