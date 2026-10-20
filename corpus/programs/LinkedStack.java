class Node {
    int value;
    Node next;
    Node(int value, Node next) { this.value = value; this.next = next; }
    public String toString() { return "Node(" + value + ")"; }
}
class Stack {
    Node top;
    int size;
    void push(int v) { top = new Node(v, top); size++; }
    int pop() {
        if (top == null) return -1;
        int v = top.value;
        top = top.next;
        size--;
        return v;
    }
    int sum() {
        int s = 0;
        Node n = top;
        while (n != null) { s += n.value; n = n.next; }
        return s;
    }
}
public class LinkedStack {
    static Node find(Node head, int k) {
        for (Node n = head; n != null; n = n.next) if (n.value == k) return n;
        return null;
    }
    public static void main(String[] a) {
        Stack s = new Stack();
        for (int i = 1; i <= 5; i++) s.push(i * i);
        System.out.println(s.sum() + " " + s.size);
        System.out.println(s.pop() + " " + s.pop());
        Node f = find(s.top, 4);
        Node g = find(s.top, 99);
        System.out.println(f + " " + g);
        System.out.println(f == s.top.next.next);
        System.out.println(g == null);
        Node h = s.top;
        System.out.println(h == s.top);
        System.out.println(h.next.value);
        Stack e = new Stack();
        System.out.println(e.pop());
        System.out.println(e.top);
        System.out.println(g.value);
    }
}
