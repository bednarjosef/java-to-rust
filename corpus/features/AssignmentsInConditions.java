// Assignments inside conditions, as HashMap's code writes them: `(e = e.next) != null` reads the
// old node through a borrow that must end before the variable is replaced, and
// `(current = e).next` reads a field through the value of an assignment of a variable that a null
// check narrowed, which is not null there. The nodes form a class tree, as HashMap's do.
public class AssignmentsInConditions {
    static class Node {
        int v;
        Node next;

        Node(int v, Node next) {
            this.v = v;
            this.next = next;
        }
    }

    static class Tree extends Node {
        Tree(int v, Node next) {
            super(v, next);
        }
    }

    static class Walk {
        Node current;
        Node next;

        Walk(Node first) {
            next = first;
        }

        Node step() {
            Node e = next;
            if (e == null) {
                throw new IllegalStateException();
            }
            if ((next = (current = e).next) == null) {
                System.out.println("last");
            }
            return e;
        }
    }

    static int sum(Node e) {
        int s = 0;
        do {
            s += e.v;
        } while ((e = e.next) != null);
        return s;
    }

    static int count(Node e) {
        int n = 0;
        while (e != null && (e = e.next) != null) {
            n++;
        }
        return n;
    }

    public static void main(String[] args) {
        Node list = new Node(1, new Tree(2, new Node(3, null)));
        System.out.println(sum(list) + " " + count(list));
        Walk w = new Walk(list);
        // A catch of `NullPointerException` sees the dereferences in `step`, and the one of the
        // narrowed variable's assignment cannot throw.
        try {
            System.out.println(w.step().v + " " + w.step().v + " " + w.step().v + " " + w.current.v);
            System.out.println(w.current.next.v);
        } catch (NullPointerException e) {
            System.out.println("null after " + w.current.v);
        }
    }
}
