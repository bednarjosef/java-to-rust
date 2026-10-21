// A ClassCastException from a downcast that a catch or a finally sees: of a value that is never
// null, of one that may be, and of an interface value; and a String.format with a literal format
// that fits its arguments, which never throws, inside a finally.
public class ClassCastCaught {
    interface Shape {
        double area();
    }

    static class Square implements Shape {
        final double side;

        Square(double side) {
            this.side = side;
        }

        public double area() {
            return side * side;
        }
    }

    static class Circle implements Shape {
        final double radius;

        Circle(double radius) {
            this.radius = radius;
        }

        public double area() {
            return 3 * radius * radius;
        }
    }

    static Shape make(int i) {
        if (i == 0) {
            return new Square(2);
        }
        return i == 1 ? new Circle(1) : null;
    }

    static String side(Shape s) {
        try {
            return "side " + ((Square) s).side;
        } catch (ClassCastException e) {
            return "not a square";
        } catch (NullPointerException e) {
            return "nothing";
        }
    }

    static String kept(Shape s) {
        try {
            Square square = (Square) s;
            return square == null ? "null kept" : "kept " + square.area();
        } catch (RuntimeException e) {
            return "cast failed";
        }
    }

    static int sum(Object[] values) {
        int total = 0;
        try {
            for (Object v : values) {
                total += (Integer) v;
            }
        } finally {
            System.out.println(String.format("summed %d values to %d", values.length, total));
        }
        return total;
    }

    public static void main(String[] args) {
        for (int i = 0; i < 3; i++) {
            System.out.println(side(make(i)) + ", " + kept(make(i)));
        }
        System.out.println(sum(new Object[] {1, 2, 3}));
        try {
            System.out.println(sum(new Object[] {1, "two", 3}));
        } catch (ClassCastException e) {
            System.out.println("caught " + e.getClass().getSimpleName());
        }
    }
}
