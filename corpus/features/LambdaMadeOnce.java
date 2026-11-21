import java.util.function.Function;
import java.util.function.Supplier;

// A lambda or method reference that captures nothing is one object per place it is written, as
// Java's runtime makes it: two evaluations of one place give the same object, two places give two
// objects, and a lambda that captures a value is a new object each time.
public class LambdaMadeOnce {
    interface Op {
        int apply(int x);
    }

    static Op twice() {
        return x -> x * 2;
    }

    static Supplier<String> greeting() {
        return LambdaMadeOnce::hello;
    }

    static String hello() {
        return "hello";
    }

    public static void main(String[] args) {
        Op a = twice(), b = twice();
        System.out.println(a == b);
        System.out.println(a.apply(21) + " " + b.apply(4));
        System.out.println(greeting() == greeting());
        System.out.println(greeting().get());
        Op first = null;
        int sum = 0;
        for (int i = 0; i < 1000; i++) {
            Op inc = x -> x + 1;
            if (first == null) {
                first = inc;
            }
            sum = inc.apply(sum);
            if (inc != first) {
                System.out.println("a new object at " + i);
            }
        }
        System.out.println(sum);
        Op one = x -> x + 1, other = x -> x + 1;
        System.out.println(one == other);
        int k = 3;
        Op c1 = x -> x + k, c2 = x -> x + k;
        System.out.println((c1 == c2) + " " + c1.apply(1));
        Function<String, Integer> length = String::length;
        System.out.println(length.apply("abcd"));
        // The one object of a place can be locked and hashed by identity.
        Op locked = twice();
        synchronized (locked) {
            System.out.println("locked " + locked.apply(5));
        }
        System.out.println(System.identityHashCode(twice()) == System.identityHashCode(locked));
    }
}
