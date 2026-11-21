// Static methods that only return a lambda of a closure type, called where the lambda is lent
// to a method that only calls it, stored, held in a local, held for a call on an object that may
// be null, called at once, from their own lambdas, with `this` or a lent closure as an argument,
// and with arguments that are literals, have effects or are assigned again: the result is the
// same whether or not the call is replaced by the lambda.
public class ClosureFactories {
    interface IntOp { int apply(int x); }

    interface Fn<T> { T apply(T t); }

    // Has a class that implements it, so it keeps the object form.
    interface Sink { void accept(int i); }

    static final class Total implements Sink {
        int sum;
        public void accept(int i) { sum += i; }
    }

    interface Visitor {
        boolean visit(int i);

        static Visitor upTo(int limit, Sink sink) {
            return i -> {
                sink.accept(i);
                return i < limit;
            };
        }
    }

    static final class Walker {
        int walk(int n, Visitor visitor) {
            int i = 0;
            while (i < n && visitor.visit(i)) i++;
            return i;
        }
    }

    static final class Loud {
        static {
            System.out.println("Loud initialized");
        }

        static IntOp twice() { return x -> 2 * x; }
    }

    static IntOp scaled(int k) { return x -> x * k; }

    static IntOp affine(int a, int b) { return x -> b + x * a; }

    static IntOp constant(int unused, int c) { return x -> c; }

    static IntOp compose(IntOp f, IntOp g) { return x -> f.apply(g.apply(x)); }

    static <T> Fn<T> constantly(T value) { return t -> value; }

    static Fn<String> prefixer(String prefix) { return s -> prefix + s; }

    static int sum(int n, IntOp op) {
        int total = 0;
        for (int i = 0; i < n; i++) total += op.apply(i);
        return total;
    }

    static String twiceApplied(Fn<String> f, String s) { return f.apply(f.apply(s)); }

    static IntOp kept;
    static int calls;

    static int next() {
        calls++;
        System.out.println("next " + calls);
        return calls;
    }

    static Walker maybe(boolean present) { return present ? new Walker() : null; }

    interface Chain { int then(Total next, int depth); }

    interface Step { int run(Total next, int depth); }

    // Factories whose lambdas call themselves, each other, and themselves from a lambda inside
    // their lambda, with an argument that is a parameter of the lambda that calls.
    static Chain chain(Total t) {
        return (next, d) -> {
            t.accept(d);
            return d <= 0 ? t.sum : chain(next).then(t, d - 1);
        };
    }

    static Chain ping(Total t) { return (next, d) -> d <= 0 ? t.sum * 10 + next.sum : pong(next).then(t, d - 1); }

    static Chain pong(Total t) {
        return (next, d) -> {
            t.accept(d);
            return ping(next).then(t, d - 1);
        };
    }

    static Chain deep(Total t) {
        return (next, d) -> {
            Step again = (n, x) -> deep(n).then(t, x);
            t.accept(d);
            return d <= 0 ? t.sum : again.run(next, d - 1);
        };
    }

    static IntOp plusOne(IntOp f) { return x -> f.apply(x) + 1; }

    // `f` is lent, and so is the lambda that captures it.
    static int sumPlusOne(int n, IntOp f) { return sum(n, plusOne(f)); }

    static final class Counted implements Sink {
        int seen;

        public void accept(int i) { seen += i; }

        int countTo(int n) { return new Walker().walk(n + 2, Visitor.upTo(n, this)) * 100 + seen; }
    }

    public static void main(String[] args) {
        System.out.println("lent " + sum(5, scaled(3)));
        int two = 2;
        int ten = 10;
        System.out.println("affine " + sum(4, affine(two, ten)) + " " + sum(4, affine(ten, two)));
        kept = scaled(7);
        System.out.println("stored " + kept.apply(6));
        int k = 2;
        IntOp before = scaled(k);
        k = 10;
        System.out.println("assigned after " + before.apply(1) + " " + scaled(k).apply(1));
        IntOp held = scaled(4);
        System.out.println("held " + (held.apply(2) + held.apply(3)));
        System.out.println("unused " + sum(2, constant(9, 4)) + " " + sum(2, constant(next(), 5)));
        System.out.println("nested " + sum(3, compose(scaled(two), affine(ten, two))));
        System.out.println("before init");
        System.out.println("initializing " + sum(3, Loud.twice()));
        System.out.println("generic " + twiceApplied(constantly("same"), "other"));
        System.out.println("literal " + twiceApplied(prefixer("> "), "quoted"));
        System.out.println("called at once " + scaled(two).apply(5));
        Total first = new Total();
        Total second = new Total();
        System.out.println("recursive " + chain(first).then(second, 3) + " " + first.sum + " " + second.sum);
        System.out.println("mutual " + ping(first).then(second, 4) + " " + first.sum + " " + second.sum);
        System.out.println("from a lambda inside " + deep(second).then(first, 5) + " " + first.sum + " " + second.sum);
        System.out.println("lent on " + sumPlusOne(3, scaled(two)));
        System.out.println("this " + new Counted().countTo(4));
        Total total = new Total();
        System.out.println("visited " + new Walker().walk(10, Visitor.upTo(4, total)) + " " + total.sum);
        for (boolean present : new boolean[] {true, false}) {
            Total seen = new Total();
            try {
                System.out.println("ordered " + maybe(present).walk(10, Visitor.upTo(next(), seen)) + " " + seen.sum);
            } catch (NullPointerException e) {
                System.out.println("caught a null walker after " + calls + " calls, " + seen.sum);
            }
            int limit = calls;
            try {
                System.out.println("held for the call " + maybe(present).walk(10, Visitor.upTo(limit, seen)) + " " + seen.sum);
            } catch (NullPointerException e) {
                System.out.println("caught a null walker with " + seen.sum);
            }
        }
    }
}
