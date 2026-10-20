// Features: java.util.function Function, BiFunction, Predicate, Supplier, Consumer, UnaryOperator,
// BinaryOperator, andThen, compose, negate, and, or, Function.identity, lambdas capturing
// effectively final locals, method references to static, instance and constructor targets
// (String::length, Integer::parseInt, StringBuilder::new, System.out::println), removeIf,
// replaceAll and forEach on a List, Map.forEach, and a user-written higher-order function.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.BiFunction;
import java.util.function.BinaryOperator;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.Supplier;
import java.util.function.UnaryOperator;

public class LambdaFunctions {
    static <T, R> List<R> mapAll(List<T> items, Function<T, R> f) {
        List<R> out = new ArrayList<>();
        for (T item : items) {
            out.add(f.apply(item));
        }
        return out;
    }

    static int applyTwice(UnaryOperator<Integer> op, int value) {
        return op.apply(op.apply(value));
    }

    static Supplier<Integer> counter(int start) {
        int[] state = {start};
        return () -> state[0]++;
    }

    static String shout(String s) {
        return s.toUpperCase() + "!";
    }

    public static void main(String[] args) {
        Function<Integer, Integer> doubled = x -> x * 2;
        Function<Integer, Integer> plusThree = x -> x + 3;
        System.out.println("andThen: " + doubled.andThen(plusThree).apply(5));
        System.out.println("compose: " + doubled.compose(plusThree).apply(5));
        System.out.println("identity: " + Function.<String>identity().apply("same"));

        Predicate<String> nonEmpty = s -> !s.isEmpty();
        Predicate<String> shortWord = s -> s.length() <= 4;
        Predicate<String> both = nonEmpty.and(shortWord);
        Predicate<String> either = nonEmpty.negate().or(shortWord.negate());
        for (String w : List.of("", "tea", "espresso")) {
            System.out.println("'" + w + "' both=" + both.test(w) + " either=" + either.test(w));
        }

        BiFunction<Integer, Integer, Integer> power = (base, exp) -> {
            int result = 1;
            for (int i = 0; i < exp; i++) {
                result *= base;
            }
            return result;
        };
        System.out.println("2^10 = " + power.apply(2, 10));
        BinaryOperator<Integer> maxOp = BinaryOperator.maxBy(java.util.Comparator.naturalOrder());
        System.out.println("maxBy: " + maxOp.apply(17, 4));

        List<String> words = new ArrayList<>(List.of("alpha", "beta", "", "gamma", "pi"));
        List<Integer> lengths = mapAll(words, String::length);
        System.out.println("lengths: " + lengths);
        List<String> shouted = mapAll(words, LambdaFunctions::shout);
        System.out.println("shouted: " + shouted);
        List<Integer> parsed = mapAll(List.of("10", "20", "-7"), Integer::parseInt);
        System.out.println("parsed sum: " + (parsed.get(0) + parsed.get(1) + parsed.get(2)));

        words.removeIf(String::isEmpty);
        words.replaceAll(String::toUpperCase);
        System.out.println("after removeIf and replaceAll: " + words);
        words.forEach(System.out::println);

        Consumer<String> printer = s -> System.out.println("consumed " + s);
        Consumer<String> twice = printer.andThen(s -> System.out.println("again " + s.length()));
        twice.accept("value");

        System.out.println("applyTwice: " + applyTwice(x -> x * x, 3));
        int offset = 100;
        System.out.println("captured offset: " + applyTwice(x -> x + offset, 1));

        Supplier<Integer> next = counter(7);
        System.out.println("counter: " + next.get() + " " + next.get() + " " + next.get());

        Supplier<StringBuilder> builders = StringBuilder::new;
        StringBuilder sb = builders.get();
        sb.append("built").append(' ').append(words.size());
        System.out.println(sb);

        Map<String, Integer> scores = new HashMap<>();
        scores.put("ann", 90);
        scores.put("bo", 72);
        scores.put("cy", 85);
        Map<String, Integer> bonus = new java.util.TreeMap<>();
        scores.forEach((name, score) -> bonus.put(name, score >= 85 ? 5 : 0));
        System.out.println("bonus: " + bonus);
        scores.replaceAll((name, score) -> score + bonus.get(name));
        System.out.println("ann now " + scores.get("ann") + ", bo now " + scores.get("bo"));
    }
}
