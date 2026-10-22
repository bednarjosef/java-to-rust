// Pattern variables that the code assigns, as javac's TransPatterns does: in a loop that walks
// a chain, after a negated test that returns, in a test joined with &&, and to null.
public class PatternAssignments {
    sealed interface Expr permits Num, Add {}
    record Num(int value) implements Expr {}
    record Add(Expr left, Expr right) implements Expr {}

    static class Node {
        final String name;
        Node next;
        Node(String name, Node next) { this.name = name; this.next = next; }
    }

    static class Special extends Node {
        Special(String name, Node next) { super(name, next); }
    }

    /** The rightmost operand of a chain of additions. */
    static Expr rightmost(Expr e) {
        if (e instanceof Add last) {
            while (last.right() instanceof Add next) {
                last = next;
            }
            return last.right();
        }
        return e;
    }

    /** The last node, starting from a special one. */
    static String last(Object o) {
        if (o instanceof Node n) {
            while (n.next != null) {
                n = n.next;
            }
            return n.name;
        }
        return "none";
    }

    static String describe(Object o) {
        if (!(o instanceof String s)) {
            return "not a string";
        }
        s = s.trim();
        return "[" + s + "]";
    }

    static int count(Object o) {
        int n = 0;
        if (o instanceof Node node) {
            for (; node != null; node = node.next) {
                n++;
            }
        }
        return n;
    }

    static int exclaimed(Object o) {
        if (o instanceof String s && !s.isEmpty()) {
            s = s + "!";
            return s.length();
        }
        return 0;
    }

    static String cleared(Object o) {
        if (o instanceof Node n) {
            n = null;
            return n == null ? "cleared" : n.name;
        }
        return "not a node";
    }

    public static void main(String[] args) {
        Expr e = new Add(new Num(1), new Add(new Num(2), new Add(new Num(3), new Num(4))));
        System.out.println(rightmost(e));
        System.out.println(rightmost(new Num(7)));
        Node chain = new Special("a", new Node("b", new Node("c", null)));
        System.out.println(last(chain));
        System.out.println(last("text"));
        System.out.println(describe("  padded  "));
        System.out.println(describe(42));
        System.out.println(count(chain));
        System.out.println(count(null));
        System.out.println(exclaimed("hey") + " " + exclaimed("") + " " + exclaimed(3));
        System.out.println(cleared(chain) + " " + cleared("x"));
    }
}
