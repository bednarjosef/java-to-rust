// A small expression language: a parser builds a tree of node classes, and an evaluator, a
// simplifier and a printer take the tree apart with instanceof patterns and casts, next to the
// virtual methods the nodes have.
public class Interpreter {
    interface Node {
        int precedence();
    }

    static class Num implements Node {
        final int value;
        Num(int value) { this.value = value; }
        public int precedence() { return 3; }
    }

    static class Var implements Node {
        final String name;
        Var(String name) { this.name = name; }
        public int precedence() { return 3; }
    }

    static abstract class Binary implements Node {
        final Node left, right;
        Binary(Node left, Node right) { this.left = left; this.right = right; }
        abstract char symbol();
    }

    static class Add extends Binary {
        Add(Node left, Node right) { super(left, right); }
        char symbol() { return '+'; }
        public int precedence() { return 1; }
    }

    static class Sub extends Binary {
        Sub(Node left, Node right) { super(left, right); }
        char symbol() { return '-'; }
        public int precedence() { return 1; }
    }

    static class Mul extends Binary {
        Mul(Node left, Node right) { super(left, right); }
        char symbol() { return '*'; }
        public int precedence() { return 2; }
    }

    static class Let implements Node {
        final String name;
        final Node value, body;
        Let(String name, Node value, Node body) { this.name = name; this.value = value; this.body = body; }
        public int precedence() { return 0; }
    }

    // Variables in scope: a stack of names and values.
    static String[] names = new String[32];
    static int[] values = new int[32];
    static int depth = 0;

    static int lookup(String name) {
        for (int i = depth - 1; i >= 0; i--) {
            if (names[i].equals(name)) return values[i];
        }
        System.out.println("unbound " + name);
        return 0;
    }

    static int eval(Node n) {
        if (n instanceof Num num) return num.value;
        if (n instanceof Var v) return lookup(v.name);
        if (n instanceof Let let) {
            int value = eval(let.value);
            names[depth] = let.name;
            values[depth] = value;
            depth++;
            int result = eval(let.body);
            depth--;
            return result;
        }
        Binary b = (Binary) n;
        int l = eval(b.left);
        int r = eval(b.right);
        if (b instanceof Add) return l + r;
        if (b instanceof Sub) return l - r;
        return l * r;
    }

    static boolean isNum(Node n, int value) {
        return n instanceof Num num && num.value == value;
    }

    static Node simplify(Node n) {
        if (n instanceof Let let) {
            Node value = simplify(let.value);
            Node body = simplify(let.body);
            if (value instanceof Num && !mentions(body, let.name)) return body;
            return new Let(let.name, value, body);
        }
        if (!(n instanceof Binary b)) {
            return n;
        }
        Node l = simplify(b.left);
        Node r = simplify(b.right);
        if (l instanceof Num x && r instanceof Num y) {
            if (b instanceof Add) return new Num(x.value + y.value);
            if (b instanceof Sub) return new Num(x.value - y.value);
            return new Num(x.value * y.value);
        }
        if (b instanceof Add) {
            if (isNum(l, 0)) return r;
            if (isNum(r, 0)) return l;
            return new Add(l, r);
        } else if (b instanceof Mul) {
            if (isNum(l, 0) || isNum(r, 0)) return new Num(0);
            if (isNum(l, 1)) return r;
            if (isNum(r, 1)) return l;
            return new Mul(l, r);
        }
        if (isNum(r, 0)) return l;
        return new Sub(l, r);
    }

    static boolean mentions(Node n, String name) {
        if (n instanceof Var v) return v.name.equals(name);
        if (n instanceof Binary b) return mentions(b.left, name) || mentions(b.right, name);
        if (n instanceof Let let) {
            return mentions(let.value, name) || (!let.name.equals(name) && mentions(let.body, name));
        }
        return false;
    }

    static String show(Node n) {
        if (n instanceof Num num) return num.value < 0 ? "(" + num.value + ")" : "" + num.value;
        if (n instanceof Var v) return v.name;
        if (n instanceof Let let) return "let " + let.name + " = " + show(let.value) + " in " + show(let.body);
        Binary b = (Binary) n;
        String l = show(b.left);
        String r = show(b.right);
        if (b.left.precedence() < b.precedence()) l = "(" + l + ")";
        // Subtraction is not associative: a right operand of the same precedence needs brackets.
        if (b.right.precedence() < b.precedence() || (b instanceof Sub && b.right.precedence() == b.precedence())) {
            r = "(" + r + ")";
        }
        return l + " " + b.symbol() + " " + r;
    }

    static int count(Node n, boolean binaries) {
        int own = (n instanceof Binary) == binaries ? 1 : 0;
        if (n instanceof Binary b) return own + count(b.left, binaries) + count(b.right, binaries);
        if (n instanceof Let let) return own + count(let.value, binaries) + count(let.body, binaries);
        return own;
    }

    // The parser: a recursive descent over the characters of the source.
    static String src;
    static int pos;

    static void skip() {
        while (pos < src.length() && src.charAt(pos) == ' ') pos++;
    }

    static boolean peek(char c) {
        skip();
        return pos < src.length() && src.charAt(pos) == c;
    }

    static String word() {
        skip();
        int start = pos;
        while (pos < src.length() && Character.isLetterOrDigit(src.charAt(pos))) pos++;
        return src.substring(start, pos);
    }

    static Node expr() {
        Node left = term();
        while (peek('+') || peek('-')) {
            char op = src.charAt(pos++);
            Node right = term();
            left = op == '+' ? new Add(left, right) : new Sub(left, right);
        }
        return left;
    }

    static Node term() {
        Node left = atom();
        while (peek('*')) {
            pos++;
            left = new Mul(left, atom());
        }
        return left;
    }

    static Node atom() {
        if (peek('(')) {
            pos++;
            Node inner = expr();
            skip();
            pos++;
            return inner;
        }
        String w = word();
        if (w.equals("let")) {
            String name = word();
            skip();
            pos++;
            Node value = expr();
            word();
            return new Let(name, value, expr());
        }
        if (Character.isDigit(w.charAt(0))) return new Num(Integer.parseInt(w));
        return new Var(w);
    }

    static Node parse(String text) {
        src = text;
        pos = 0;
        return expr();
    }

    public static void main(String[] args) {
        String[] programs = {
            "1 + 2 * 3",
            "(1 + 2) * 3",
            "let x = 4 in x * x - 0",
            "let y = 2 + 3 in (y - 1) * 1 + 0 * y",
            "10 - (4 - 3) - 2",
            "let a = 3 in let b = a * 2 in b - a",
            "let unused = 7 in 2 * (5 - 5)",
            "q + 1",
        };
        for (String p : programs) {
            Node tree = parse(p);
            Node simple = simplify(tree);
            System.out.println(show(tree) + " => " + show(simple));
            System.out.println("  value " + eval(tree) + " / " + eval(simple) + ", nodes "
                    + count(tree, true) + "+" + count(tree, false) + " -> " + count(simple, true) + "+" + count(simple, false));
        }
    }
}
