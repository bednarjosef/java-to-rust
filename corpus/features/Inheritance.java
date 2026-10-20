// A class extending an abstract class: inherited fields and methods, an abstract method that the
// subclass implements, an inherited toString, upcasts to the superclass type and identity.
public class Inheritance {
    abstract static class Shape {
        private final String name;
        protected int moves;
        private int[] history = new int[4];

        Shape(String name) {
            this.name = name;
        }

        String name() {
            return name;
        }

        void move(int by) {
            history[moves % history.length] = by;
            moves++;
        }

        int lastMove() {
            return moves == 0 ? 0 : history[(moves - 1) % history.length];
        }

        abstract double area();

        String describe() {
            return name + " with area " + area() + " moved " + moves + " times";
        }

        @Override
        public String toString() {
            return "Shape(" + name + ", " + moves + ")";
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
            return 3.0 * radius * radius;
        }

        double radius() {
            return radius;
        }
    }

    // Values that never leave their local: the subclass's object is a plain value.
    abstract static class Counter {
        int count;

        void bump() {
            count++;
        }
    }

    static class Clicks extends Counter {
        int clicks;

        void click() {
            clicks++;
            bump();
        }
    }

    // A class tree whose objects never change after construction.
    abstract static class Quantity {
        final long amount;

        Quantity(long amount) {
            this.amount = amount;
        }

        abstract String unit();

        String show() {
            return amount + " " + unit();
        }
    }

    static final class Grams extends Quantity {
        Grams(long amount) {
            super(amount);
        }

        @Override
        String unit() {
            return "g";
        }

        Grams plus(Grams other) {
            return new Grams(amount + other.amount);
        }
    }

    static void push(Shape shape, int times) {
        for (int i = 0; i < times; i++) {
            shape.move(i + 1);
        }
    }

    static Shape biggest(Shape a, Shape b) {
        return a.area() >= b.area() ? a : b;
    }

    public static void main(String[] args) {
        Circle c = new Circle(2.0);
        c.move(5);
        push(c, 3);
        Shape s = c;
        s.move(7);
        System.out.println(s.name() + " " + s.area() + " " + c.radius());
        System.out.println(c.describe());
        System.out.println(s);
        System.out.println("last " + s.lastMove() + ", same " + (s == c));
        Circle d = new Circle(1.0);
        Shape big = biggest(d, c);
        System.out.println(big == c ? "the first circle" : "the second circle");
        Shape[] shapes = {c, d};
        double total = 0;
        for (Shape shape : shapes) {
            total += shape.area();
        }
        System.out.println("total " + total);

        Clicks clicks = new Clicks();
        clicks.click();
        clicks.click();
        clicks.bump();
        System.out.println(clicks.count + " " + clicks.clicks);

        Grams g = new Grams(250).plus(new Grams(500));
        Quantity q = g;
        System.out.println(q.show() + " " + g.amount);
    }
}
