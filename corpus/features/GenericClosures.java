import java.util.function.BiFunction;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.Supplier;
import java.util.function.UnaryOperator;

// Generic functional interfaces whose values are only lambdas: closures over the erased types,
// the lambda's body behind the conversions of its bridge. UnaryOperator extends Function, so
// both stay enums here.
public class GenericClosures {
    interface Transform<T> {
        T apply(T value);
    }

    interface Task<T> {
        T run(int attempt) throws Exception;
    }

    interface Pair<A, B> {
        B combine(A first, A second);
    }

    private final String prefix;
    private final Function<String, String> decorate;

    GenericClosures(String prefix) {
        this.prefix = prefix;
        this.decorate = s -> this.prefix + s;
    }

    String show(String s) {
        return decorate.apply(s);
    }

    static <T> T twice(Transform<T> f, T x) {
        return f.apply(f.apply(x));
    }

    static int sign(Function<Integer, Integer> f, int x) {
        int y = f.apply(x);
        if (y < 0) {
            return -1;
        }
        return y == 0 ? 0 : 1;
    }

    static <T> T retry(Task<T> task) throws Exception {
        for (int i = 0; ; i++) {
            try {
                return task.run(i);
            } catch (IllegalStateException e) {
                System.out.println("retrying after " + e.getMessage());
            }
        }
    }

    static int sum(Pair<Integer, Integer> p, int a, int b) {
        return p.combine(a, b);
    }

    public static void main(String[] args) throws Exception {
        Function<Integer, Integer> square = x -> x * x;
        System.out.println(square.apply(7));
        int k = 4;
        Function<Integer, Integer> early = x -> {
            if (x > k) {
                return x - k;
            }
            return k - x;
        };
        System.out.println(early.apply(1) + " " + early.apply(9));
        Supplier<String> hello = () -> "hello";
        System.out.println(hello.get());
        Supplier<Integer> answer = () -> 6 * 7;
        int a = answer.get();
        System.out.println(a + 1);
        BiFunction<String, Integer, String> repeat = (s, n) -> s.repeat(n);
        System.out.println(repeat.apply("ab", 3));
        Predicate<String> empty = String::isEmpty;
        System.out.println(empty.test("") + " " + empty.test("x"));
        StringBuilder sb = new StringBuilder();
        Consumer<String> append = s -> sb.append(s).append(';');
        append.accept("one");
        append.accept("two");
        System.out.println(sb);
        Transform<String> exclaim = s -> s + "!";
        System.out.println(twice(exclaim, "hi"));
        System.out.println(twice(x -> x + 1, 5));
        System.out.println(sign(x -> x - 10, 3) + " " + sign(x -> x, 0));

        GenericClosures g = new GenericClosures("> ");
        System.out.println(g.show("hello"));
        UnaryOperator<String> upper = String::toUpperCase;
        System.out.println(upper.apply("shout"));
        int limit = 2;
        Task<String> flaky = attempt -> {
            if (attempt < limit) {
                throw new IllegalStateException("attempt " + attempt);
            }
            return "done on " + attempt;
        };
        System.out.println(retry(flaky));
        System.out.println(sum((x, y) -> x * 10 + y, 4, 2));
        try {
            retry(attempt -> {
                throw new Exception("never");
            });
        } catch (Exception e) {
            System.out.println("gave up: " + e.getMessage());
        }
    }
}
