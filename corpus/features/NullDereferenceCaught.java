// A NullPointerException from calling a method or reading a field through a null reference, seen
// by a catch of it or of a superclass, and by a finally that runs before it goes on.
public class NullDereferenceCaught {
    static class Node {
        int value;
        Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }

        int value() {
            return value;
        }
    }

    static Node find(Node head, int value) {
        for (Node n = head; n != null; n = n.next) {
            if (n.value == value) {
                return n;
            }
        }
        return null;
    }

    static String describe(Node head, int value) {
        try {
            return "found " + find(head, value).value();
        } catch (NullPointerException e) {
            return "missing " + value;
        }
    }

    static int secondValue(Node head) {
        try {
            return head.next.value;
        } catch (RuntimeException e) {
            return -1;
        } finally {
            System.out.println("looked at the second node");
        }
    }

    static int chained(Node head) {
        return head.next.next.value();
    }

    public static void main(String[] args) {
        Node list = new Node(1, new Node(2, new Node(3, null)));
        System.out.println(describe(list, 2));
        System.out.println(describe(list, 7));
        System.out.println(secondValue(list));
        System.out.println(secondValue(new Node(5, null)));
        try {
            System.out.println(chained(list));
            System.out.println(chained(list.next));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getClass().getName());
        }
        Throwable t = new IllegalStateException("outer", null);
        try {
            System.out.println(t.getCause().getMessage());
        } catch (Exception e) {
            System.out.println("no cause");
        } finally {
            System.out.println("done");
        }
    }
}
