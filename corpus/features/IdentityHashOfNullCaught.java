// `System.identityHashCode` of a value that may be null, where a `catch` would see a
// `NullPointerException`: it is 0 for null and throws nothing, as the JDK's
// `HashMap.TreeNode.tieBreakOrder` relies on.
public class IdentityHashOfNullCaught {
    static final class Node {
        final int key;

        Node(int key) {
            this.key = key;
        }
    }

    static int tieBreakOrder(Object a, Object b) {
        int d;
        if (a == null || b == null || (d = a.getClass().getName().compareTo(b.getClass().getName())) == 0) {
            d = (System.identityHashCode(a) <= System.identityHashCode(b) ? -1 : 1);
        }
        return d;
    }

    static boolean sameHash(Node n, Node m) {
        return System.identityHashCode(n) == System.identityHashCode(m);
    }

    public static void main(String[] args) {
        Object[] xs = {"a", Integer.valueOf(3), null};
        Node node = new Node(1);
        Node[] nodes = {node, null, new Node(2)};
        for (int i = 0; i < xs.length; i++) {
            try {
                System.out.println(tieBreakOrder(xs[i], "b") != 0);
                System.out.println(tieBreakOrder(null, xs[i]));
                System.out.println(System.identityHashCode(xs[i]) == 0);
                System.out.println(sameHash(nodes[i], nodes[i]) + " " + sameHash(nodes[i], node));
                System.out.println(System.identityHashCode(nodes[i]) == 0);
            } catch (NullPointerException e) {
                System.out.println("null");
            }
        }
    }
}
