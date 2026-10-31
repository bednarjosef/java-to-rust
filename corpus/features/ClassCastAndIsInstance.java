import java.util.function.Function;
import java.util.function.Predicate;

public class ClassCastAndIsInstance {
    interface Shape { double area(); }
    static abstract class Base implements Shape {
        public String toString() { return getClass().getSimpleName(); }
    }
    static class Circle extends Base {
        public double area() { return 3.0; }
    }
    static class Square extends Base {
        public double area() { return 4.0; }
    }
    enum Color { RED, GREEN }
    // An enum whose constants have bodies, each a class below the enum.
    enum Op {
        PLUS { int apply(int a, int b) { return a + b; } },
        TIMES { int apply(int a, int b) { return a * b; } };
        abstract int apply(int a, int b);
    }
    static final class Point { }

    static Comparable<?> first = Op.PLUS;

    static Comparable<?> firstKey(Comparable<?>[] keys) {
        return keys[0];
    }

    static Class<?> kindOf(String why) {
        System.out.println("kind " + why);
        return Op.class;
    }

    static Comparable<?> keyOf(String why, Comparable<?>[] keys, int i) {
        System.out.println("key " + why);
        return keys[i];
    }

    static <T> T as(Class<T> type, Object o) {
        if (!type.isInstance(o)) {
            return null;
        }
        return type.cast(o);
    }

    static String describe(Class<?> type, Object o) {
        return type.getSimpleName() + "(" + o + ")=" + type.isInstance(o);
    }

    public static void main(String[] args) {
        Object[] values = { new Circle(), new Square(), "text", 42, Color.RED, null, new int[] { 1 }, new String[] { "a" } };
        Class<?>[] types = { Shape.class, Base.class, Circle.class, String.class, CharSequence.class, Integer.class,
            Number.class, Comparable.class, Color.class, Enum.class, Object.class, int[].class, Object[].class };
        for (Class<?> type : types) {
            StringBuilder line = new StringBuilder(type.getSimpleName() + ":");
            for (Object value : values) {
                line.append(' ').append(type.isInstance(value) ? 1 : 0);
            }
            System.out.println(line);
        }
        Shape s = as(Shape.class, values[0]);
        System.out.println(s.area());
        System.out.println(as(Square.class, values[0]));
        System.out.println("square " + as(Square.class, values[1]));
        Circle c = Circle.class.cast(values[0]);
        System.out.println(c.area() + " " + describe(Circle.class, c));
        System.out.println(String.class.cast(null) + " " + Integer.class.cast(values[3]));
        Predicate<Object> isBase = Base.class::isInstance;
        Function<Object, Base> toBase = Base.class::cast;
        int n = 0;
        for (Object v : values) {
            if (isBase.test(v)) {
                n++;
                System.out.println("base " + toBase.apply(v));
            }
        }
        System.out.println(n);
        Class<? extends Shape> k = Square.class;
        Shape sq = k.cast(values[1]);
        System.out.println(sq.area());
        // Classes that nothing extends, and an enum whose constants are classes below it.
        Object[] more = { Op.PLUS, Op.TIMES, new Point(), Color.GREEN, true, 'c', new ArithmeticException("x") };
        Class<?>[] leaves = { Op.class, Point.class, Color.class, Boolean.class, Character.class, Square.class,
            ArithmeticException.class, RuntimeException.class };
        for (Class<?> type : leaves) {
            StringBuilder line = new StringBuilder(type.getSimpleName() + ":");
            for (Object value : more) {
                line.append(' ').append(type.isInstance(value) ? 1 : 0);
            }
            System.out.println(line);
        }
        System.out.println(Op.class.cast(more[1]).apply(6, 7) + " " + Color.class.cast(more[3]));
        // Variables of a class with no subclass, and one that is null: their class numbers are known.
        Point point = new Point();
        Point none = args.length > 5 ? point : null;
        Object held = args.length > 5 ? null : Color.RED;
        System.out.println(Point.class.isInstance(point) + " " + Point.class.isInstance(none) + " "
            + Color.class.isInstance(point) + " " + Object.class.isInstance(point) + " "
            + Color.class.isInstance(held) + " " + Op.class.isInstance(held) + " " + (Point.class.cast(none) == null));
        // Values that are not variables: elements, a field, a call's result, and the class first.
        Comparable<?>[] keys = { Color.GREEN, Op.TIMES, 7, null, "k" };
        Class<?>[] kinds = { Color.class, Op.class, Integer.class, Color.class, Op.class };
        StringBuilder tests = new StringBuilder("elements:");
        for (int i = 0; i < keys.length; i++) {
            tests.append(' ').append(kinds[i].isInstance(keys[i]) ? 1 : 0)
                .append(kinds[(i + 1) % kinds.length].isInstance(keys[i]) ? 1 : 0);
        }
        System.out.println(tests + " " + Op.class.isInstance(first) + Color.class.isInstance(firstKey(keys))
            + " " + kindOf("class").isInstance(keyOf("value", keys, 1)));
        System.out.println(Op.class.cast(keys[1]) + " " + Color.class.cast(keys[3]));
        // A null class throws after the value is evaluated, where a catch sees it.
        Class<?> noKind = args.length > 5 ? Op.class : null;
        try {
            System.out.println(noKind.isInstance(keyOf("after the class", keys, 0)));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        Object some = keys[2];
        try {
            System.out.println(noKind.isInstance(some));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(noKind.cast(keyOf("before the cast", keys, 1)));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(noKind.cast(some) + " " + noKind.cast(null));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            System.out.println(1 / (args.length));
        } catch (RuntimeException e) {
            System.out.println(ArithmeticException.class.isInstance(e) + " " + RuntimeException.class.isInstance(e));
        }
        try {
            System.out.println(Point.class.cast(more[3]));
        } catch (ClassCastException e) {
            System.out.println("caught: " + e.getMessage());
        }
        Object wrong = values[2];
        try {
            Square square = Square.class.cast(values[0]);
            System.out.println("not reached " + square.area());
        } catch (ClassCastException e) {
            System.out.println("caught: " + e.getMessage());
        }
        Circle bad = Circle.class.cast(wrong);
        System.out.println("not reached " + bad.area());
    }
}
