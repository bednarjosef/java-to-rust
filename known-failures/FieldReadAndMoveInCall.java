public class FieldReadAndMoveInCall {
    static class Node {
        int v;
        Node next;
        Node(int v, Node next) { this.v = v; this.next = next; }
    }

    static int depth(Node n) {
        return n == null ? 0 : 1 + depth(n.next);
    }

    public static void main(String[] args) {
        Node single = new Node(1, null);
        single.next = null;
        System.out.println(depth(single.next) + " " + depth(single));
    }
}
