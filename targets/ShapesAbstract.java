// Features: abstract class with abstract and concrete methods, subclasses with super(...) calls,
// polymorphic dispatch through an array and a List, instanceof checks and casts, static counters
// in a class, protected fields, toString overrides, String.format with %.2f, Math.PI and Math.sqrt.
import java.util.ArrayList;
import java.util.List;

public class ShapesAbstract {
    abstract static class Shape {
        private static int created = 0;
        protected final String label;

        Shape(String label) {
            this.label = label;
            created++;
        }

        abstract double area();

        abstract double perimeter();

        String describe() {
            return String.format("%-10s area=%8.2f perimeter=%7.2f", label, area(), perimeter());
        }

        static int createdCount() {
            return created;
        }

        @Override
        public String toString() {
            return "Shape[" + label + "]";
        }
    }

    static class Circle extends Shape {
        private final double radius;

        Circle(double radius) {
            super("circle");
            this.radius = radius;
        }

        @Override
        double area() {
            return Math.PI * radius * radius;
        }

        @Override
        double perimeter() {
            return 2 * Math.PI * radius;
        }
    }

    static class Rectangle extends Shape {
        protected final double width;
        protected final double height;

        Rectangle(double width, double height) {
            this("rectangle", width, height);
        }

        protected Rectangle(String label, double width, double height) {
            super(label);
            this.width = width;
            this.height = height;
        }

        @Override
        double area() {
            return width * height;
        }

        @Override
        double perimeter() {
            return 2 * (width + height);
        }
    }

    static class Square extends Rectangle {
        Square(double side) {
            super("square", side, side);
        }

        double side() {
            return width;
        }

        @Override
        public String toString() {
            return "Square[" + side() + "]";
        }
    }

    static class Triangle extends Shape {
        private final double a;
        private final double b;
        private final double c;

        Triangle(double a, double b, double c) {
            super("triangle");
            if (a + b <= c || a + c <= b || b + c <= a) {
                throw new IllegalArgumentException("not a triangle: " + a + ", " + b + ", " + c);
            }
            this.a = a;
            this.b = b;
            this.c = c;
        }

        @Override
        double area() {
            double s = perimeter() / 2;
            return Math.sqrt(s * (s - a) * (s - b) * (s - c));
        }

        @Override
        double perimeter() {
            return a + b + c;
        }
    }

    public static void main(String[] args) {
        List<Shape> shapes = new ArrayList<>();
        shapes.add(new Circle(1.5));
        shapes.add(new Rectangle(3, 4.5));
        shapes.add(new Square(2));
        shapes.add(new Triangle(3, 4, 5));
        try {
            shapes.add(new Triangle(1, 2, 10));
        } catch (IllegalArgumentException e) {
            System.out.println("skipped: " + e.getMessage());
        }

        for (Shape s : shapes) {
            System.out.println(s.describe());
        }

        double total = 0;
        Shape largest = null;
        for (Shape s : shapes) {
            total += s.area();
            if (largest == null || s.area() > largest.area()) {
                largest = s;
            }
        }
        System.out.printf("total area %.3f%n", total);
        System.out.println("largest: " + largest);

        int rectangles = 0;
        for (Shape s : shapes) {
            if (s instanceof Rectangle) {
                rectangles++;
                Rectangle r = (Rectangle) s;
                System.out.println(r.label + " is wide? " + (r.width > r.height));
            }
        }
        System.out.println("rectangles incl. squares: " + rectangles);
        System.out.println("shapes created: " + Shape.createdCount());
        Object o = shapes.get(2);
        System.out.println("square side via cast: " + ((Square) o).side());
        try {
            Circle wrong = (Circle) o;
            System.out.println(wrong);
        } catch (ClassCastException e) {
            System.out.println("cast failed as expected");
        }
    }
}
