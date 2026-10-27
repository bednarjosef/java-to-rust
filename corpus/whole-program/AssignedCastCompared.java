// A cast assigned to a variable that may be null, compared as it is assigned, as StampedLock's
// acquireRead does with `(leader = (ReaderNode) t) == tail`: the assignment's value is the cast.
public class AssignedCastCompared {
    static class Node { int status; }
    static final class ReaderNode extends Node { }
    static final class WriterNode extends Node { }
    Node tail;
    int acquire(Node t) {
        ReaderNode leader = null;
        if (t == null) {
            return 0;
        } else if (t instanceof WriterNode) {
            return 3;
        } else if ((leader = (ReaderNode) t) == tail) {
            return 1 + leader.status;
        }
        return leader == null ? -1 : 2;
    }
    public static void main(String[] args) {
        AssignedCastCompared m = new AssignedCastCompared();
        ReaderNode r = new ReaderNode();
        m.tail = r;
        System.out.println(m.acquire(null) + " " + m.acquire(r) + " " + m.acquire(new ReaderNode()) + " " + m.acquire(new WriterNode()));
    }
}
