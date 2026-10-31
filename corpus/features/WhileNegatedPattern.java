// `while (!(x instanceof T t)) { .. }`: the loop ends only where the pattern matched, so `t` is in
// scope after it, as `XWindow.getParentWindowID` walks up its parents to the first window.
public class WhileNegatedPattern {
    static class Node {
        final Node parent;
        final String name;

        Node(Node parent, String name) {
            this.parent = parent;
            this.name = name;
        }
    }

    static class Window extends Node {
        final int id;

        Window(Node parent, String name, int id) {
            super(parent, name);
            this.id = id;
        }
    }

    static int windowOf(Node start) {
        Node at = start;
        int steps = 0;
        while (!(at instanceof Window window)) {
            at = at.parent;
            steps++;
            if (steps > 100) {
                continue;
            }
        }
        System.out.println("found " + window.name + " after " + steps + " steps");
        return window.id;
    }

    static String firstText(Object[] items) {
        int i = 0;
        Object item = items[0];
        while (!(item instanceof String text)) {
            i++;
            item = items[i];
        }
        return text + " at " + i;
    }

    public static void main(String[] args) {
        Window root = new Window(null, "root", 7);
        Node a = new Node(root, "a");
        Node b = new Node(a, "b");
        Window inner = new Window(b, "inner", 9);
        Node c = new Node(inner, "c");
        System.out.println(windowOf(c));
        System.out.println(windowOf(b));
        System.out.println(windowOf(root));
        System.out.println(firstText(new Object[] {1, 2.5, 'c', "text", "later"}));
    }
}
