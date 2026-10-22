// A toString that throws, called where the program makes an object's text: in a concatenation, a
// print (to System.err too), String.format, String.valueOf, a builder's append and the `+=` of an
// array element and of a static; of a class alone and of classes under a common type, null or
// not, inside a `try` that catches what it throws and on the way through a `finally`; and in the
// message of an exception, where it ends the program. As javac's Type.toString may throw anything
// that its callers catch.
public class CaughtToStringFailures {
    static class Unprintable extends RuntimeException {
        Unprintable(String message) {
            super(message);
        }
    }

    static class Cell {
        final int value;

        Cell(int value) {
            this.value = value;
        }

        @Override
        public String toString() {
            if (value < 0) {
                throw new Unprintable("negative cell " + value);
            }
            return "cell " + value;
        }
    }

    // Its message shows a node, whose toString may throw: that ends the program.
    static class Unshown extends RuntimeException {
        Unshown(Node node) {
            super("unshown " + node);
        }
    }

    abstract static class Node {
        abstract int size();
    }

    static class Leaf extends Node {
        int size() {
            return 1;
        }

        @Override
        public String toString() {
            return "leaf";
        }
    }

    static class Broken extends Node {
        final String why;

        Broken(String why) {
            this.why = why;
        }

        int size() {
            return 0;
        }

        @Override
        public String toString() {
            throw new IllegalStateException("broken: " + why);
        }
    }

    static class Pair extends Node {
        final Node left;
        final Node right;

        Pair(Node left, Node right) {
            this.left = left;
            this.right = right;
        }

        int size() {
            return left.size() + right.size();
        }

        @Override
        public String toString() {
            return "(" + left + " " + right + ")";
        }
    }

    static String describe(Cell cell) {
        try {
            return "described " + cell;
        } catch (Unprintable e) {
            return "undescribable: " + e.getMessage();
        }
    }

    static String show(Node node) {
        try {
            return String.valueOf(node);
        } catch (IllegalStateException e) {
            return "unshowable (" + e.getMessage() + ")";
        }
    }

    static String pair(Node first, Node second) {
        try {
            return "pair " + first + " " + second;
        } catch (IllegalStateException e) {
            return "no pair: " + e.getMessage();
        }
    }

    static String formatted(Node left, Node right) {
        try {
            return String.format("%s and %s", left, right);
        } catch (IllegalStateException e) {
            System.err.println(left);
            return "unformattable: " + e.getMessage();
        }
    }

    static int printed = 0;

    static void print(Node node) {
        try {
            System.out.println(node);
            printed++;
        } finally {
            System.out.println("printed so far: " + printed);
        }
    }

    static String[] logs = {"log:", "log:"};
    static String last = "last:";

    static void log(int slot, Node node) {
        try {
            logs[slot] += " " + node;
            logs[slot] += node;
            last += node;
        } catch (IllegalStateException e) {
            logs[slot] += " (" + e.getMessage() + ")";
        }
    }

    public static void main(String[] args) {
        System.out.println(describe(new Cell(3)));
        System.out.println(describe(new Cell(-2)));
        System.out.println(describe(null));
        Node[] nodes = {
            new Leaf(), new Pair(new Leaf(), new Leaf()), new Pair(new Leaf(), new Broken("left out")), new Broken("alone")
        };
        for (Node node : nodes) {
            System.out.println(node.size() + ": " + show(node));
        }
        System.out.println(pair(nodes[0], null));
        System.out.println(pair(null, nodes[3]));
        System.out.println(formatted(nodes[0], nodes[1]));
        System.out.println(formatted(nodes[1], nodes[3]));
        System.out.println(formatted(nodes[3], nodes[0]));
        String text = "start";
        try {
            text += " " + nodes[1];
            text += " " + nodes[3];
        } catch (IllegalStateException e) {
            System.out.println("text stays " + text + " after " + e.getMessage());
        }
        StringBuilder built = new StringBuilder("cells:");
        for (int value : new int[] {1, -1, 2}) {
            try {
                built.append(' ').append(new Cell(value));
            } catch (Unprintable e) {
                built.append(" (").append(e.getMessage()).append(')');
            }
        }
        System.out.println(built);
        for (int i = 0; i < nodes.length; i++) {
            log(i % 2, nodes[i]);
        }
        System.out.println(logs[0] + " / " + logs[1] + " / " + last);
        for (Node node : nodes) {
            try {
                print(node);
            } catch (IllegalStateException e) {
                System.out.println("not printed: " + e.getMessage());
            }
        }
        try {
            throw new Unshown(nodes[1]);
        } catch (Unshown e) {
            System.out.println(e.getMessage());
        }
        throw new Unshown(nodes[3]);
    }
}
