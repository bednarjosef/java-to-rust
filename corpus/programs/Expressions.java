public class Expressions {
    static abstract class Node {
        static int created;
        Node() { created++; }
        abstract int eval();
        abstract String show();
        int depth() { return 1; }
        public String toString() { return show() + " = " + eval(); }
    }

    static class Num extends Node {
        final int value;
        Num(int value) { this.value = value; }
        int eval() { return value; }
        String show() { return "" + value; }
    }

    static abstract class Binary extends Node {
        final Node left, right;
        Binary(Node left, Node right) { this.left = left; this.right = right; }
        abstract String op();
        abstract int apply(int a, int b);
        int eval() { return apply(left.eval(), right.eval()); }
        String show() { return "(" + left.show() + " " + op() + " " + right.show() + ")"; }
        int depth() { return 1 + Math.max(left.depth(), right.depth()); }
    }

    static class Add extends Binary {
        Add(Node l, Node r) { super(l, r); }
        String op() { return "+"; }
        int apply(int a, int b) { return a + b; }
    }

    static class Mul extends Binary {
        Mul(Node l, Node r) { super(l, r); }
        String op() { return "*"; }
        int apply(int a, int b) { return a * b; }
    }

    static class Neg extends Node {
        final Node inner;
        Neg(Node inner) { this.inner = inner; }
        int eval() { return -inner.eval(); }
        String show() { return "-" + inner.show(); }
        int depth() { return 1 + inner.depth(); }
    }

    static Node build(int n) {
        if (n <= 1) return new Num(n + 1);
        if (n % 3 == 0) return new Neg(build(n - 1));
        if (n % 2 == 0) return new Add(build(n - 1), new Num(n));
        return new Mul(build(n / 2), build(n - 2));
    }

    static Node simplify(Node n) {
        if (n.depth() > 3) return new Num(n.eval());
        return n;
    }

    public static void main(String[] args) {
        Node e = new Add(new Num(2), new Mul(new Num(3), new Num(4)));
        System.out.println(e);
        System.out.println(e.depth());
        for (int i = 1; i <= 7; i++) {
            Node b = build(i);
            System.out.println(i + ": " + b + " depth " + b.depth());
            System.out.println("   simplified: " + simplify(b));
        }
        Binary top = new Mul(e, new Neg(new Num(5)));
        System.out.println(top.left.eval() + " " + top.right + " " + top.op());
        System.out.println(Node.created);
    }
}
