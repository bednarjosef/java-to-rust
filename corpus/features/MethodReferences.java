import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.IntBinaryOperator;
import java.util.function.IntUnaryOperator;
import java.util.function.Supplier;
import java.util.function.ToIntFunction;
import java.util.function.UnaryOperator;

// Method references of every kind that captures nothing: static methods of the program and of
// the library, unbound instance methods (`String::length`), constructors of the program's classes
// and of `StringBuilder`, array constructors, `System.out::println`, and overloaded methods that
// the descriptor picks.
public class MethodReferences {
    interface Maker {
        Point make(int x, int y);
    }

    interface Measure {
        int of(Point p);
    }

    interface Arrays2 {
        int[] make(int size);
    }

    interface Names {
        String[] make(int size);
    }

    interface Builders {
        StringBuilder make();
    }

    interface Printer {
        void print(String s);
    }

    interface Joiner {
        String join(String a, String b);
    }

    static class Point {
        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        Point() {
            this(0, 0);
        }

        int manhattan() {
            return Math.abs(x) + Math.abs(y);
        }

        Point mirrored() {
            return new Point(y, x);
        }

        public String toString() {
            return "(" + x + ", " + y + ")";
        }
    }

    static int twice(int a) {
        return a * 2;
    }

    static int combine(int a, int b) {
        return a * 10 + b;
    }

    static String describe(int n) {
        return "int " + n;
    }

    static String describe(String s) {
        return "string " + s;
    }

    static String bracket(String a, String b) {
        return "[" + a + "|" + b + "]";
    }

    public static void main(String[] args) {
        IntUnaryOperator doubler = MethodReferences::twice;
        IntBinaryOperator combiner = MethodReferences::combine;
        System.out.println(doubler.applyAsInt(21) + " " + combiner.applyAsInt(4, 2));
        IntBinaryOperator larger = Math::max;
        IntUnaryOperator magnitude = Math::abs;
        System.out.println(larger.applyAsInt(3, 9) + " " + magnitude.applyAsInt(-7));

        Function<String, Integer> length = String::length;
        ToIntFunction<String> parse = Integer::parseInt;
        UnaryOperator<String> upper = String::toUpperCase;
        UnaryOperator<String> trimmed = String::trim;
        System.out.println(length.apply("seven") + parse.applyAsInt("35") + " " + upper.apply("loud"));
        System.out.println("[" + trimmed.apply("  padded  ") + "]");
        BiFunction<String, String, Boolean> starts = String::startsWith;
        BiFunction<String, Integer, Character> charAt = String::charAt;
        System.out.println(starts.apply("prefix", "pre") + " " + starts.apply("prefix", "fix") + " " + charAt.apply("hello", 1));

        Maker maker = Point::new;
        Supplier<Point> origin = Point::new;
        Point p = maker.make(3, -4);
        System.out.println(p + " " + origin.get());
        Measure measure = Point::manhattan;
        Function<Point, Point> mirror = Point::mirrored;
        System.out.println(measure.of(p) + " " + mirror.apply(p));

        Builders builders = StringBuilder::new;
        StringBuilder sb = builders.make();
        sb.append("built").append(1);
        System.out.println(sb);

        Arrays2 ints = int[]::new;
        int[] made = ints.make(4);
        made[2] = 9;
        System.out.println(made.length + " " + made[2] + " " + made[0]);
        Names names = String[]::new;
        String[] labels = names.make(2);
        labels[0] = "first";
        System.out.println(labels.length + " " + labels[0]);

        Printer printer = System.out::println;
        printer.print("printed by reference");
        Function<Integer, String> intText = MethodReferences::describe;
        Function<String, String> stringText = MethodReferences::describe;
        System.out.println(intText.apply(5) + ", " + stringText.apply("five"));
        Joiner joiner = MethodReferences::bracket;
        Joiner concat = String::concat;
        System.out.println(joiner.join("a", "b") + " " + concat.join("con", "cat"));
    }
}
