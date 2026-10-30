// Many small objects of sizes between the multiples of 16 bytes: 2 million list nodes of two
// `long`s and a link (24 bytes in the translation) and 1 million records of five `long`s (40),
// all alive at once, and then 30 million nodes that die young. What it measures is the memory the
// live ones take (`scripts/benchmark.py` reports the peak) and the time allocation takes.
public class SmallObjects {
    static class Node {
        long key;
        long value;
        Node next;

        Node(long key, long value, Node next) {
            this.key = key;
            this.value = value;
            this.next = next;
        }
    }

    static class Record {
        long a;
        long b;
        long c;
        long d;
        long e;

        Record(long seed) {
            a = seed;
            b = seed * 3;
            c = seed ^ 0x5555;
            d = seed + 7;
            e = seed * seed;
        }
    }

    public static void main(String[] args) {
        Node head = null;
        for (int i = 0; i < 2_000_000; i++) {
            head = new Node(i, 2L * i, head);
        }
        Record[] records = new Record[1_000_000];
        for (int i = 0; i < records.length; i++) {
            records[i] = new Record(i);
        }
        long sum = 0;
        for (Node n = head; n != null; n = n.next) {
            n.value += n.key;
            sum += n.value;
        }
        for (Record r : records) {
            sum += r.a + r.b + r.c + r.d + r.e;
        }
        // Short-lived nodes: each lives for one step of the loop.
        Node last = null;
        for (int i = 0; i < 30_000_000; i++) {
            Node n = new Node(i, sum & 0xff, last);
            sum += n.value + (n.next == null ? 0 : n.next.key);
            last = (i & 1023) == 0 ? null : new Node(i, 1, null);
        }
        System.out.println(sum);
    }
}
