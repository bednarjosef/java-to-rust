class Range {
    int lo, hi;
    Range(int a, int b) {
        lo = a; hi = b;
        if (a <= b) return;
        lo = b; hi = a;
        return;
    }
}
class Node {
    static int made;
    Node next;
    int v;
    Node(int v, Node next) {
        this.v = v;
        this.next = next;
        made++;
        if (next == null) return;
        next.v += v;
    }
}
public class ConstructorReturns {
    public static void main(String[] args) {
        Range r = new Range(5, 2);
        Range q = new Range(1, 3);
        System.out.println(r.lo + " " + r.hi + " " + q.lo + " " + q.hi);
        Node n = new Node(1, new Node(2, null));
        System.out.println(n.v + " " + n.next.v + " " + n.made + " " + Node.made);
    }
}
