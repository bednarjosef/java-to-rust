// A functional interface with default and static methods whose values are only lambdas: a
// closure type, its default methods an extension trait of the closure.
public class ClosureDefaultMethods {
    interface Check {
        boolean test(int x);

        default Check and(Check other) {
            return x -> test(x) && other.test(x);
        }

        default Check negate() {
            return x -> !test(x);
        }

        default Check orNegated(Check other) {
            return x -> test(x) || other.negate().test(x);
        }

        static Check not(Check c) {
            return c.negate();
        }

        default String describe(int x) {
            return x + (test(x) ? " passes" : " fails");
        }
    }

    public static void main(String[] args) {
        Check even = x -> x % 2 == 0;
        Check positive = x -> x > 0;
        Check both = even.and(positive);
        System.out.println(both.test(4) + " " + both.test(-4) + " " + both.test(3));
        System.out.println(even.negate().test(3) + " " + Check.not(positive).test(-1));
        System.out.println(even.orNegated(positive).test(-3));
        System.out.println(both.describe(6) + ", " + both.describe(7));
    }
}
