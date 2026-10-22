// A pattern chain `o instanceof Pair p && v.visit(..) && ..` whose operands unbox a `Boolean`
// that may be null: the `NullPointerException` of the unboxing reaches the `catch`.
public class UnboxingInPatternChainCaught {
    interface Visitor { Boolean visit(int a, int b); }

    static Boolean maybe(int a, int b) { return a < 0 ? null : a == b; }

    record Pair(int a, int b) {
        boolean same(Object o, Visitor v) {
            return (o instanceof Pair p) && v.visit(a, p.a) && v.visit(b, p.b);
        }

        // Only the inner `&&` unboxes: the last operand is a `boolean`.
        boolean first(Object o, Visitor v, boolean also) {
            return o instanceof Pair p && v.visit(a, p.a) && also;
        }
    }

    public static void main(String[] args) {
        Visitor v = UnboxingInPatternChainCaught::maybe;
        try {
            System.out.println(new Pair(1, 2).same(new Pair(1, 2), v));
            System.out.println(new Pair(1, 2).same("x", v));
            System.out.println(new Pair(-1, 2).same(new Pair(-1, 2), v));
        } catch (NullPointerException e) {
            System.out.println("npe");
        }
        try {
            System.out.println(new Pair(1, 2).first(new Pair(1, 2), v, true));
            System.out.println(new Pair(1, 2).first(new Pair(1, 5), v, false));
            System.out.println(new Pair(-1, 2).first(new Pair(-1, 2), v, true));
        } catch (NullPointerException e) {
            System.out.println("npe again");
        }
    }
}
