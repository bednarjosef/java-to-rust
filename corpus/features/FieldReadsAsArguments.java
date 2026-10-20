public class FieldReadThenPass {
    static class Node {
        int v;
        Node next;
        Node(int v, Node next) { this.v = v; this.next = next; }
    }

    static int depth(Node n) {
        return n == null ? 0 : 1 + depth(n.next);
    }

    public static void main(String[] args) {
        Node a = new Node(1, new Node(2, null));
        System.out.println(depth(a.next) + " " + depth(a));
    }
}
