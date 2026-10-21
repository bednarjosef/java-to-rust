// The default and static methods of java.util.function: composed functions and predicates.
import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.IntPredicate;
import java.util.function.Predicate;
import java.util.function.UnaryOperator;

public class FunctionChains {
    public static void main(String[] args) {
        Function<Integer, Integer> twice = x -> x * 2;
        Function<Integer, Integer> plusOne = x -> x + 1;
        System.out.println(twice.andThen(plusOne).apply(5) + " " + twice.compose(plusOne).apply(5));
        Function<String, String> same = Function.identity();
        System.out.println(same.apply("same"));
        UnaryOperator<String> upper = String::toUpperCase;
        System.out.println(upper.andThen(s -> s + "!").apply("loud"));
        BiFunction<Integer, Integer, Integer> add = (a, b) -> a + b;
        System.out.println(add.andThen(twice).apply(3, 4));

        Predicate<String> empty = String::isEmpty;
        Predicate<String> longer = s -> s.length() > 3;
        System.out.println(empty.negate().test("") + " " + empty.or(longer).test("abcd") + " " + empty.negate().and(longer).test("ab"));
        System.out.println(Predicate.not(empty).test("x") + " " + Predicate.isEqual("a").test("a"));
        IntPredicate even = n -> n % 2 == 0;
        IntPredicate positive = n -> n > 0;
        System.out.println(even.and(positive).test(4) + " " + even.or(positive).test(-3) + " " + even.negate().test(3));
    }
}
