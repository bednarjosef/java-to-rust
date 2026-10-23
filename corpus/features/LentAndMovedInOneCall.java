// One call that lends a variable to a borrowed parameter and, in another argument, passes it by
// value in the condition of a `?:`: the condition may not move it while the call borrows it.
public class LentAndMovedInOneCall {
    static final class Node {
        final String name;

        Node(String name) {
            this.name = name;
        }
    }

    static Node kept;

    static boolean keep(Node n) {
        kept = n;
        return n != null;
    }

    static String show(Node n, String suffix) {
        return (n == null ? "null" : n.name) + suffix;
    }

    static String both(Node n) {
        return show(n, keep(n) ? "!" : "?");
    }

    public static void main(String[] args) {
        System.out.println(both(new Node("a")));
        System.out.println(both(null));
        System.out.println(kept == null);
    }
}
