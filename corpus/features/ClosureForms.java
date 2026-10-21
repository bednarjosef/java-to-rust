// Functional interfaces whose values are only lambdas and method references, which nothing sees
// as objects: their values are Rust closures.
public class ClosureForms {
    interface IntOp {
        int apply(int a, int b);
    }

    interface Action {
        void run();
    }

    interface Check {
        boolean test(String s) throws Exception;
    }

    interface Maker {
        Counter make(int start);
    }

    static class Counter {
        int count;
        final IntOp step;

        Counter(int start) {
            count = start;
            step = (a, b) -> a + b;
        }

        void add(int n) {
            count = step.apply(count, n);
        }
    }

    static int total = 0;
    int base = 10;
    IntOp scale = (a, b) -> a * b;

    static int fold(int[] xs, int start, IntOp op) {
        int acc = start;
        for (int x : xs) {
            acc = op.apply(acc, x);
        }
        return acc;
    }

    static IntOp offset(int by) {
        return (a, b) -> a + b + by;
    }

    int scaled(int x) {
        return scale.apply(x, base);
    }

    void demo() {
        IntOp withBase = (a, b) -> a + scaled(b);
        System.out.println(withBase.apply(1, 2));
        base = 3;
        System.out.println(withBase.apply(1, 2));
        Action bump = () -> base++;
        bump.run();
        bump.run();
        System.out.println(base);
    }

    static boolean check(Check c, String s) {
        try {
            return c.test(s);
        } catch (Exception e) {
            System.out.println("failed: " + e.getMessage());
            return false;
        }
    }

    public static void main(String[] args) {
        int k = 5;
        String name = "sum";
        IntOp add = (a, b) -> a + b;
        IntOp plusK = (a, b) -> a + b + k;
        IntOp max = Math::max;
        int[] xs = {3, 1, 4, 1, 5};
        System.out.println(name + " " + fold(xs, 0, add) + " " + fold(xs, 0, plusK) + " " + fold(xs, 0, max));
        System.out.println(offset(100).apply(1, 2) + " " + fold(xs, 0, offset(-1)));
        Action hello = () -> System.out.println("hello " + name);
        hello.run();
        Action count = () -> total += k;
        count.run();
        count.run();
        System.out.println(total);
        IntOp[] ops = {add, max, (a, b) -> a * b};
        for (IntOp op : ops) {
            System.out.println(op.apply(6, 7));
        }
        new ClosureForms().demo();
        Maker maker = Counter::new;
        Counter c = maker.make(7);
        c.add(3);
        System.out.println(c.count);
        Check nonEmpty = s -> {
            if (s.isEmpty()) {
                throw new Exception("empty");
            }
            return true;
        };
        System.out.println(check(nonEmpty, "x") + " " + check(nonEmpty, ""));
        IntOp twice = (a, b) -> {
            IntOp inner = (x, y) -> x * k + y;
            return inner.apply(a, b) * 2;
        };
        System.out.println(twice.apply(2, 3));
    }
}
