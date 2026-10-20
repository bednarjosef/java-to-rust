// Features: an expression tree as an abstract class with subclasses (Num, Var, Add, Sub, Mul,
// Div, Neg), recursive eval over a Map<String, Integer> environment, a custom checked exception
// for undefined variables, an unchecked ArithmeticException for division by zero, toString with
// parentheses, simplification with instanceof checks and casts, and equals on the tree.
import java.util.HashMap;
import java.util.Map;

public class ExpressionTree {
    static class UndefinedVariableException extends Exception {
        UndefinedVariableException(String name) {
            super("undefined variable " + name);
        }
    }

    abstract static class Expr {
        abstract int eval(Map<String, Integer> env) throws UndefinedVariableException;

        abstract Expr simplify();
    }

    static class Num extends Expr {
        final int value;

        Num(int value) {
            this.value = value;
        }

        int eval(Map<String, Integer> env) {
            return value;
        }

        Expr simplify() {
            return this;
        }

        @Override
        public String toString() {
            return Integer.toString(value);
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Num n && n.value == value;
        }

        @Override
        public int hashCode() {
            return value;
        }
    }

    static class Var extends Expr {
        final String name;

        Var(String name) {
            this.name = name;
        }

        int eval(Map<String, Integer> env) throws UndefinedVariableException {
            Integer v = env.get(name);
            if (v == null) {
                throw new UndefinedVariableException(name);
            }
            return v;
        }

        Expr simplify() {
            return this;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    abstract static class Binary extends Expr {
        final Expr left;
        final Expr right;

        Binary(Expr left, Expr right) {
            this.left = left;
            this.right = right;
        }

        abstract String symbol();

        @Override
        public String toString() {
            return "(" + left + " " + symbol() + " " + right + ")";
        }
    }

    static class Add extends Binary {
        Add(Expr l, Expr r) {
            super(l, r);
        }

        String symbol() {
            return "+";
        }

        int eval(Map<String, Integer> env) throws UndefinedVariableException {
            return left.eval(env) + right.eval(env);
        }

        Expr simplify() {
            Expr l = left.simplify();
            Expr r = right.simplify();
            if (l instanceof Num a && a.value == 0) {
                return r;
            }
            if (r instanceof Num b && b.value == 0) {
                return l;
            }
            return new Add(l, r);
        }
    }

    static class Sub extends Binary {
        Sub(Expr l, Expr r) {
            super(l, r);
        }

        String symbol() {
            return "-";
        }

        int eval(Map<String, Integer> env) throws UndefinedVariableException {
            return left.eval(env) - right.eval(env);
        }

        Expr simplify() {
            return new Sub(left.simplify(), right.simplify());
        }
    }

    static class Mul extends Binary {
        Mul(Expr l, Expr r) {
            super(l, r);
        }

        String symbol() {
            return "*";
        }

        int eval(Map<String, Integer> env) throws UndefinedVariableException {
            return left.eval(env) * right.eval(env);
        }

        Expr simplify() {
            Expr l = left.simplify();
            Expr r = right.simplify();
            if (l instanceof Num a && a.value == 1) {
                return r;
            }
            if (r instanceof Num b && b.value == 1) {
                return l;
            }
            return new Mul(l, r);
        }
    }

    static class Div extends Binary {
        Div(Expr l, Expr r) {
            super(l, r);
        }

        String symbol() {
            return "/";
        }

        int eval(Map<String, Integer> env) throws UndefinedVariableException {
            int divisor = right.eval(env);
            return left.eval(env) / divisor;
        }

        Expr simplify() {
            return new Div(left.simplify(), right.simplify());
        }
    }

    static class Neg extends Expr {
        final Expr inner;

        Neg(Expr inner) {
            this.inner = inner;
        }

        int eval(Map<String, Integer> env) throws UndefinedVariableException {
            return -inner.eval(env);
        }

        Expr simplify() {
            Expr s = inner.simplify();
            if (s instanceof Neg twice) {
                return twice.inner;
            }
            return new Neg(s);
        }

        @Override
        public String toString() {
            return "-" + inner;
        }
    }

    static void evaluate(String label, Expr e, Map<String, Integer> env) {
        try {
            System.out.println(label + ": " + e + " = " + e.eval(env));
        } catch (UndefinedVariableException ex) {
            System.out.println(label + ": error: " + ex.getMessage());
        } catch (ArithmeticException ex) {
            System.out.println(label + ": arithmetic error: " + ex.getMessage());
        }
    }

    public static void main(String[] args) {
        Map<String, Integer> env = new HashMap<>();
        env.put("x", 6);
        env.put("y", -3);

        Expr a = new Add(new Mul(new Num(2), new Var("x")), new Neg(new Var("y")));
        Expr b = new Div(new Sub(new Var("x"), new Num(1)), new Add(new Var("y"), new Num(3)));
        Expr c = new Add(new Num(0), new Mul(new Num(1), new Var("z")));
        Expr d = new Neg(new Neg(new Add(new Var("x"), new Num(0))));

        evaluate("a", a, env);
        evaluate("b", b, env);
        evaluate("c", c, env);
        evaluate("d", d, env);
        evaluate("d simplified", d.simplify(), env);
        System.out.println("simplified c: " + c.simplify());
        System.out.println("simplified d: " + d.simplify());

        env.put("z", 11);
        evaluate("c with z", c, env);
        evaluate("c simplified with z", c.simplify(), env);
        System.out.println("equal nums: " + new Num(4).equals(new Num(4)) + " " + new Num(4).equals(new Var("four")));
        System.out.println("hash of 9: " + new Num(9).hashCode());
    }
}
