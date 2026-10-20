public class NullChecks {
    static class Node {
        int val;
        Node next;
        Node(int val, Node next) { this.val = val; this.next = next; }
    }

    static Node find(Node head, int target) {
        Node n = head;
        while (n != null && n.val != target) n = n.next;
        return n;
    }

    static int valueOr(Node n, int dflt) {
        return n == null ? dflt : n.val;
    }

    static boolean bigAndLast(Node n) {
        return n != null && n.val > 3 && (n.next == null || n.next.val > n.val);
    }

    static boolean eitherMissing(Node a, Node b) {
        return a == null || b == null || a.val == b.val;
    }

    static int length(Node n) {
        int len = 0;
        while (n != null) {
            len++;
            n = n.next;
        }
        return len;
    }

    static Node lastOrNull(Node n) {
        if (n == null) return null;
        while (n.next != null) n = n.next;
        return n;
    }

    public static void main(String[] args) {
        Node list = new Node(1, new Node(5, new Node(9, null)));
        System.out.println(find(list, 5).val);
        System.out.println(find(list, 4) == null);
        System.out.println(valueOr(find(list, 4), -1) + " " + valueOr(list, -1));
        System.out.println(bigAndLast(list) + " " + bigAndLast(list.next) + " " + bigAndLast(null));
        System.out.println(bigAndLast(new Node(4, new Node(2, null))));
        System.out.println(eitherMissing(null, list) + " " + eitherMissing(list, list.next));
        System.out.println(eitherMissing(list.next.next, list.next.next));
        System.out.println(length(list) + " " + length(null) + " " + length(list.next));
        System.out.println(lastOrNull(list).val + " " + (lastOrNull(null) == null));
        Node cur = list;
        int sum = 0;
        while (cur != null && sum < 100) {
            sum += cur.val;
            cur = cur.next;
        }
        System.out.println(sum + " " + (cur == null));
        Node missing = null;
        boolean ok = missing != null && missing.val == 0;
        boolean also = missing == null || missing.val == 0;
        System.out.println(ok + " " + also);
        Node[] slots = new Node[3];
        slots[1] = list;
        int count = 0;
        for (int i = 0; i < slots.length; i++) {
            if (slots[i] != null && slots[i].next != null) count++;
        }
        System.out.println("count=" + count);
        System.out.println((slots[0] == null) + " " + (slots[1] != null ? slots[1].val : -1));
        Node tail = null;
        for (int i = 0; i < 4; i++) {
            tail = new Node(i * i, tail);
        }
        System.out.println(length(tail) + " " + valueOr(tail, 0) + " " + tail.val);
        System.out.println(tail.next.next.next == null);
    }
}
