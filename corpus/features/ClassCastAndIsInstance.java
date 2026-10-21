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
