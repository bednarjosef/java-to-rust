// Iterable.forEach on a class of the program, Consumer.andThen, the static methods of
// BinaryOperator, UnaryOperator and Objects that take functions, and a lambda that throws.
import java.util.Iterator;
import java.util.Objects;
import java.util.function.BinaryOperator;
import java.util.function.Consumer;
import java.util.function.IntBinaryOperator;
import java.util.function.Supplier;
import java.util.function.UnaryOperator;

public class Consumers {
    static class Range implements Iterable<Integer> {
        final int from;
        final int to;

        Range(int from, int to) {
            this.from = from;
            this.to = to;
        }

        public Iterator<Integer> iterator() {
            return new Iterator<Integer>() {
                int next = from;

                public boolean hasNext() {
                    return next < to;
                }

                public Integer next() {
                    return next++;
                }
            };
        }
    }

    static int total = 0;

    public static void main(String[] args) {
        Range range = new Range(1, 5);
        range.forEach(i -> total += i);
        System.out.println(total);
        StringBuilder log = new StringBuilder();
        Consumer<String> first = s -> log.append("first ").append(s).append(';');
        Consumer<String> both = first.andThen(s -> log.append("second ").append(s).append(';'));
        both.accept("x");
        System.out.println(log);
        Supplier<String> fallback = () -> "fallback";
        String name = null;
        System.out.println(Objects.requireNonNullElse(name, "else") + " " + Objects.requireNonNullElseGet(name, fallback));
        System.out.println(Objects.requireNonNull("present", "missing"));
        BinaryOperator<String> shorter = BinaryOperator.minBy((a, b) -> a.length() - b.length());
        BinaryOperator<String> longer = BinaryOperator.maxBy((a, b) -> a.length() - b.length());
        System.out.println(shorter.apply("abc", "de") + " " + longer.apply("abc", "de"));
        UnaryOperator<String> same = UnaryOperator.identity();
        System.out.println(same.apply("same"));
        IntBinaryOperator max = Math::max;
        System.out.println(max.applyAsInt(3, 9));
        try {
            Objects.requireNonNull(name, () -> "no name");
        } catch (NullPointerException e) {
            System.out.println("caught no name");
        }
        Consumer<Integer> check = i -> {
            if (i < 0) {
                throw new IllegalArgumentException("negative " + i);
            }
            System.out.println("ok " + i);
        };
        try {
            check.accept(2);
            check.accept(-1);
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
