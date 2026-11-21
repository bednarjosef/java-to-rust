// Fields read again where nothing between could change them, in the shapes the Minecraft server's
// code has: a reference field tested for null and then used (`if (this.vehicle != null)
// this.vehicle.onPassengerTurned(this)` in `Entity.turn`), the same with an early return and in a
// conditional expression, a field stored and read back (`Level.getBlockRandomPos` updates
// `randValue` and reads it again), and a field read twice in one expression or in the next
// statement. Java's JIT loads each such field once. The fields are written after construction and
// the objects are used on a worker thread, as the server's are, so they are fields that threads
// may share.
public class FieldRereads {
    static final class Node {
        int value;
        int weight;
        Node next;

        Node(int value, int weight) {
            this.value = value;
            this.weight = weight;
        }

        int weigh(int scale) {
            return this.weight * scale + this.value;
        }
    }

    static final class Mover {
        Node vehicle;
        Node target;
        Node leader;
        int randValue = 1;
        int sizeBits = 4;
        int moved;
        int turns;

        // `Entity.turn`: a test, then a call on the tested field.
        void turn(int by) {
            this.turns += by;
            if (this.vehicle != null) {
                this.moved += this.vehicle.weigh(by);
            }
        }

        // A test, then reads of the tested field's fields.
        int follow() {
            if (this.target != null && this.target.value > 0) {
                return this.target.value + this.target.weight;
            }
            return 0;
        }

        // An early return, then reads of the field.
        int lead() {
            if (this.leader == null) {
                return -1;
            }
            int distance = this.leader.value - this.leader.weight;
            return distance + (this.leader.next == null ? 0 : this.leader.next.value);
        }

        // `Level.getBlockRandomPos`: a field stored, then read back.
        int randomPos(int x, int y, int z) {
            this.randValue = this.randValue * 3 + 1013904223;
            int val = this.randValue >> 2;
            return (x + (val & 15)) ^ (y + (val >> 16 & 15)) ^ (z + (val >> 8 & 15));
        }

        // `PalettedContainer.Strategy.getIndex`: one field twice in one expression.
        int index(int x, int y, int z) {
            return (y << this.sizeBits | z) << this.sizeBits | x;
        }

        // A conditional expression on a field.
        int targetValue() {
            return this.target == null ? 0 : this.target.value;
        }
    }

    static long run(Mover[] movers, Node[] nodes, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int m = 0; m < movers.length; m++) {
                Mover mover = movers[m];
                mover.turn(r & 7);
                sum += mover.follow();
                sum += mover.lead();
                sum += mover.index(mover.randomPos(r, m, 5) & 15, r & 15, m & 15);
                sum += mover.targetValue();
            }
            if ((r & 1023) == 0) {
                // Fields that change after construction, now and then to null.
                for (int m = 0; m < movers.length; m++) {
                    Mover mover = movers[m];
                    int k = (r >> 10) + m;
                    mover.vehicle = k % 5 == 0 ? null : nodes[k % nodes.length];
                    mover.target = k % 7 == 0 ? null : nodes[(k * 3) % nodes.length];
                    mover.leader = k % 11 == 0 ? null : nodes[(k * 5) % nodes.length];
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws InterruptedException {
        int rounds = args.length > 0 ? Integer.parseInt(args[0]) : 400000;
        Node[] nodes = new Node[64];
        for (int i = 0; i < nodes.length; i++) {
            nodes[i] = new Node(i * 7 - 100, i % 9);
        }
        for (int i = 0; i < nodes.length; i++) {
            nodes[i].next = i % 3 == 0 ? null : nodes[(i * 13) % nodes.length];
        }
        Mover[] movers = new Mover[256];
        for (int m = 0; m < movers.length; m++) {
            movers[m] = new Mover();
            movers[m].sizeBits = 4 + m % 2;
        }
        long[] result = new long[2];
        Thread worker = new Thread(() -> {
            result[0] = run(movers, nodes, 2000);
            long start = System.nanoTime();
            result[1] = run(movers, nodes, rounds);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        worker.start();
        worker.join();
        long moved = 0;
        for (Mover mover : movers) {
            moved += mover.moved + mover.turns + mover.randValue;
        }
        System.out.println("field rereads " + result[0] + " " + result[1] + " " + moved);
    }
}
