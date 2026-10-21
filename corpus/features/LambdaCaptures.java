import java.util.function.Function;
import java.util.function.IntSupplier;
import java.util.function.IntUnaryOperator;
import java.util.function.Supplier;

// Lambdas and method references that capture: locals and parameters (also of generic methods
// and loops), the enclosing object (`this`, its fields and methods named without it, `this::m`),
// a lambda inside a lambda, lambdas in field initializers and constructors, in local and
// anonymous classes, and a recursive lambda that calls itself through a field.
public class LambdaCaptures {
    interface Op {
        int apply(int a);
    }

    interface Counter {
        int next();
    }

    private int base = 10;
    private int count;
    private final String label;
    private final Counter counter = () -> ++count;
    private final Function<Integer, Integer> factorial = n -> n <= 1 ? 1 : n * this.factorial.apply(n - 1);

    LambdaCaptures(String label) {
        this.label = label;
        Runnable announce = () -> System.out.println("made " + this.label);
        announce.run();
    }

    int scaled(int x) {
        return x * base;
    }

    Op adder() {
        return a -> a + base;
    }

    Op scaler() {
        return this::scaled;
    }

    Supplier<String> labeler() {
        return () -> label + "!" + this.base;
    }

    static Op offset(int k) {
        return a -> a + k;
    }

    static <T> Supplier<T> constant(T value) {
        return () -> value;
    }

    Counter stepping(int step) {
        class Stepper implements Counter {
            int at;

            public int next() {
                IntSupplier s = () -> at += step;
                return s.getAsInt() + count;
            }
        }
        return new Stepper();
    }

    Counter anonymous(int step) {
        return new Counter() {
            int at = 100;

            public int next() {
                Function<Integer, Integer> add = x -> x + step + at;
                at = add.apply(0);
                return at;
            }
        };
    }

    public static void main(String[] args) {
        int k = 3;
        Op plus = a -> a + k;
        System.out.println(plus.apply(4));
        String name = "abc";
        Function<String, String> greet = s -> s + " " + name;
        System.out.println(greet.apply("hello"));

        LambdaCaptures c = new LambdaCaptures("first");
        Op add = c.adder();
        c.base = 20;
        System.out.println(add.apply(1) + " " + c.scaler().apply(2) + " " + c.labeler().get());
        System.out.println(offset(5).apply(6) + " " + constant("same").get() + " " + constant(7).get());
        System.out.println(c.counter.next() + " " + c.counter.next() + " " + c.factorial.apply(5));

        Counter stepper = c.stepping(5);
        System.out.println(stepper.next() + " " + stepper.next());
        Counter anonymous = c.anonymous(3);
        System.out.println(anonymous.next() + " " + anonymous.next());

        Supplier<Integer> length = name::length;
        System.out.println(length.get());
        IntUnaryOperator nested = x -> {
            int y = x * 2;
            IntUnaryOperator inner = z -> z + y + k;
            return inner.applyAsInt(1);
        };
        System.out.println(nested.applyAsInt(5));

        int[] counted = {0};
        Runnable increment = () -> counted[0]++;
        increment.run();
        increment.run();
        System.out.println(counted[0]);

        String[] words = {"x", "yy", "zzz"};
        int total = 0;
        for (String w : words) {
            Supplier<Integer> size = w::length;
            total += size.get();
        }
        System.out.println(total);
        for (int i = 0; i < 3; i++) {
            final int j = i;
            Runnable print = () -> System.out.print(j + ";");
            print.run();
        }
        System.out.println();
    }
}
