import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.IntBinaryOperator;
import java.util.function.Predicate;
import java.util.function.Supplier;

// Lambdas that capture nothing, as objects of the interfaces they convert to: a program interface
// with lambdas and a class implementing it, kept in an array and applied in a loop, lambdas
// returned from methods and stored in fields, a generic program interface, and the JDK's
// functional interfaces.
public class LambdaObjects {
    interface IntOp {
        int apply(int a, int b);
    }

    static class Power implements IntOp {
        public int apply(int a, int b) {
            int result = 1;
            for (int i = 0; i < b; i++) {
                result *= a;
            }
            return result;
        }
    }

    interface Transform<A, B> {
        B apply(A value);
    }

    interface Check {
        boolean test(String s);
    }

    static final IntOp MAX = (a, b) -> a > b ? a : b;

    static IntOp named(String name) {
        switch (name) {
            case "add":
                return (a, b) -> a + b;
            case "sub":
                return (a, b) -> a - b;
            default:
                return (a, b) -> {
                    System.out.println("unknown operator");
                    return 0;
                };
        }
    }

    static int fold(int[] values, int start, IntOp op) {
        int acc = start;
        for (int v : values) {
            acc = op.apply(acc, v);
        }
        return acc;
    }

    static <A, B> B applyTwice(Transform<A, A> first, Transform<A, B> then, A value) {
        return then.apply(first.apply(first.apply(value)));
    }

    static int count(String[] words, Check check) {
        int n = 0;
        for (String w : words) {
            if (check.test(w)) {
                n++;
            }
        }
        return n;
    }

    public static void main(String[] args) {
        IntOp[] ops = {(a, b) -> a + b, (a, b) -> a * b, new Power(), MAX, (a, b) -> {
            int lo = Math.min(a, b);
            int hi = Math.max(a, b);
            return hi % lo;
        }};
        for (IntOp op : ops) {
            System.out.println(op.apply(3, 4) + " " + op.apply(7, 2));
        }
        int[] values = {5, 1, 4, 2, 3};
        System.out.println(fold(values, 0, named("add")));
        System.out.println(fold(values, 100, named("sub")));
        System.out.println(fold(values, 0, MAX));
        System.out.println(fold(values, 1, ops[1]));
        System.out.println(named("mod").apply(1, 2));

        Transform<Integer, Integer> doubled = x -> x * 2;
        Transform<Integer, String> shown = x -> "<" + x + ">";
        System.out.println(applyTwice(doubled, shown, 5));
        Transform<String, String> exclaimed = s -> s + "!";
        Transform<String, Integer> length = s -> s.length();
        System.out.println(applyTwice(exclaimed, length, "hey"));

        String[] words = {"apple", "kiwi", "banana", "fig", "cherry"};
        System.out.println(count(words, s -> s.length() > 4));
        System.out.println(count(words, s -> s.indexOf('i') >= 0));
        System.out.println(count(words, s -> {
            for (int i = 0; i < s.length(); i++) {
                if (s.charAt(i) == 'a') {
                    return true;
                }
            }
            return false;
        }));

        Runnable hello = () -> System.out.println("hello from a Runnable");
        hello.run();
        hello.run();
        Supplier<String> greeting = () -> "supplied " + 42;
        System.out.println(greeting.get());
        Function<Integer, Integer> square = x -> x * x;
        System.out.println(square.apply(12));
        BiFunction<String, Integer, String> repeat = (s, n) -> s.repeat(n);
        System.out.println(repeat.apply("ab", 3));
        Predicate<String> empty = s -> s.isEmpty();
        System.out.println(empty.test("") + " " + empty.test("x"));
        IntBinaryOperator gcd = (a, b) -> {
            while (b != 0) {
                int t = a % b;
                a = b;
                b = t;
            }
            return a;
        };
        System.out.println(gcd.applyAsInt(84, 36));
    }
}
