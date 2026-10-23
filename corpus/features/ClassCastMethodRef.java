import java.util.function.Function;

public class ClassCastMethodRef {
    static class Point {
        final int x;

        Point(int x) {
            this.x = x;
        }

        @Override
        public String toString() {
            return "Point(" + x + ")";
        }
    }

    static <T, R> String mapAll(Object[] items, Function<T, R> f) {
        StringBuilder out = new StringBuilder();
        for (Object item : items) {
            try {
                out.append(f.apply((T) item)).append(';');
            } catch (ClassCastException e) {
                out.append("caught: ").append(e.getMessage()).append(';');
            }
        }
        return out.toString();
    }

    public static void main(String[] args) {
        Function<Object, String> asString = String.class::cast;
        System.out.println(asString.apply("hello"));
        try {
            System.out.println(asString.apply(42));
        } catch (ClassCastException e) {
            System.out.println("caught: " + e.getMessage());
        }

        Object[] strings = {"a", "b", "c"};
        System.out.println(mapAll(strings, String.class::cast));

        Function<Object, Point> asPoint = Point.class::cast;
        System.out.println(asPoint.apply(new Point(3)));
        try {
            System.out.println(asPoint.apply("nope"));
        } catch (ClassCastException e) {
            System.out.println("caught: " + e.getMessage());
        }

        Object[] mixed = {new Point(1), "two", new Point(3)};
        System.out.println(mapAll(mixed, Point.class::cast));

        System.out.println(asString.apply(null));
        System.out.println(asPoint.apply(null));
    }
}
