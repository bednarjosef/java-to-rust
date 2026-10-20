public class RecursiveObjects {
    static class Node {
        int val;
        Node next;
        Node(int val, Node next) { this.val = val; this.next = next; }
    }

    static Node reverse(Node n) {
        if (n == null || n.next == null) return n;
        Node rest = reverse(n.next);
        n.next.next = n;
        n.next = null;
        return rest;
    }

    static int sum(Node n) {
        return n == null ? 0 : n.val + sum(n.next);
    }

    static String show(Node n) {
        if (n == null) return "nil";
        return n.val + " -> " + show(n.next);
    }

    static Node insertSorted(Node head, Node n) {
        if (head == null || n.val < head.val) {
            n.next = head;
            return n;
        }
        head.next = insertSorted(head.next, n);
        return head;
    }

    static Node dropEvens(Node n) {
        if (n == null) return null;
        Node rest = dropEvens(n.next);
        if (n.val % 2 == 0) return rest;
        n.next = rest;
        return n;
    }

    static int countPaths(int[][] cells, int r, int c, Node trail) {
        if (r >= cells.length || c >= cells[0].length) return 0;
        Node here = new Node(cells[r][c], trail);
        if (r == cells.length - 1 && c == cells[0].length - 1) {
            System.out.println("path " + show(here));
            return 1;
        }
        return countPaths(cells, r + 1, c, here) + countPaths(cells, r, c + 1, here);
    }

    static void shift(Node[] ns, int i) {
        if (i == ns.length) return;
        ns[i] = new Node(i * 3, i > 0 ? ns[i - 1] : null);
        shift(ns, i + 1);
    }

    public static void main(String[] args) {
        Node list = new Node(1, new Node(2, new Node(3, new Node(4, null))));
        System.out.println(show(list) + " sum " + sum(list));
        Node r = reverse(list);
        System.out.println(show(r));
        System.out.println(list.next == null ? "old head is tail" : "old head linked");
        Node sorted = null;
        int[] vals = { 5, 1, 4, 2, 3 };
        for (int v : vals) sorted = insertSorted(sorted, new Node(v, null));
        System.out.println(show(sorted));
        sorted = dropEvens(sorted);
        System.out.println(show(sorted));
        int[][] cells = { { 1, 2, 3 }, { 4, 5, 6 } };
        System.out.println("paths " + countPaths(cells, 0, 0, null));
        Node[] ns = new Node[4];
        shift(ns, 0);
        System.out.println(ns[3].val + " " + ns[3].next.val + " " + (ns[0].next == null));
    }
}
