// Cursor loops (`while (n != null) n = n.next;`) over a list whose nodes are never
// changed after construction, so the translator keeps them as plain shared values.
public class ListCursorLoops {
    static class Node {
        final int value;
        final Node next;

        Node(int value, Node next) {
            this.value = value;
            this.next = next;
        }
    }

    static Node build(int... values) {
        Node head = null;
        for (int i = values.length - 1; i >= 0; i--) head = new Node(values[i], head);
        return head;
    }

    static int length(Node n) {
        int count = 0;
        while (n != null) {
            count++;
            n = n.next;
        }
        return count;
    }

    static Node last(Node n) {
        Node cur = n;
        while (cur != null && cur.next != null) cur = cur.next;
        return cur;
    }

    static Node nth(Node n, int index) {
        Node cur = n;
        for (int i = 0; i < index && cur != null; i++) cur = cur.next;
        return cur;
    }

    static boolean contains(Node n, int value) {
        for (Node cur = n; cur != null; cur = cur.next) {
            if (cur.value == value) return true;
        }
        return false;
    }

    static int sum(Node n) {
        int total = 0;
        Node cur = n;
        while (cur != null) {
            total += cur.value;
            cur = cur.next;
        }
        return total;
    }

    static String render(Node n) {
        String out = "[";
        for (Node cur = n; cur != null; cur = cur.next) {
            out = out + cur.value + (cur.next == null ? "" : ",");
        }
        return out + "]";
    }

    static Node firstAtLeast(Node n, int bound) {
        Node cur = n;
        while (cur != null && cur.value < bound) {
            cur = cur.next;
        }
        return cur;
    }

    static Node dropBelow(Node n, int bound) {
        Node cur = n;
        while (cur != null && cur.value < bound) cur = cur.next;
        return cur;
    }

    public static void main(String[] args) {
        Node list = build(4, 8, 15, 16, 23, 42);
        System.out.println(length(list) + " " + sum(list) + " " + render(list));
        System.out.println(last(list).value + " " + nth(list, 2).value + " " + (nth(list, 9) == null));
        System.out.println(contains(list, 23) + " " + contains(list, 5));
        Node tail = firstAtLeast(list, 16);
        int tailValue = tail.value;
        System.out.println(tailValue + " " + length(tail) + " " + render(tail));
        System.out.println(dropBelow(list, 100) == null);
        Node none = build();
        System.out.println(length(none) + " " + render(none) + " " + (last(none) == null));
        Node single = build(7);
        System.out.println(last(single).value + " " + render(single));
    }
}
