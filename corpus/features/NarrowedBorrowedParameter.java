// A parameter that may be null, which the method borrows, passed on to another such parameter
// inside a null check that narrowed it, and outside one.
public class NarrowedBorrowedParameter {
    static final class Node {
        final String name;
        Node next;

        Node(String name, Node next) {
            this.name = name;
            this.next = next;
        }
    }

    static int depth(Node n) {
        return n == null ? 0 : 1 + depth(n.next);
    }

    static int count(Node a, Node b) {
        int c = 0;
        if (a != null) c++;
        if (b != null) c++;
        return c;
    }

    static String describe(Node n) {
        if (n != null) {
            return n.name + ":" + depth(n) + ":" + count(n, n.next);
        }
        return "none:" + depth(n) + ":" + count(n, null);
    }

    static String label(Node n) {
        if (n == null) {
            return "null";
        } else {
            return n.name + count(n.next, n);
        }
    }

    public static void main(String[] args) {
        Node list = new Node("a", new Node("b", new Node("c", null)));
        System.out.println(describe(null));
        System.out.println(describe(list));
        System.out.println(describe(list.next.next));
        System.out.println(label(null));
        System.out.println(label(list.next));
    }
}
