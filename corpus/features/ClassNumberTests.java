// Tests of an object's class as the translation writes them, a compare of its class number: one
// class, a range of subclasses, a value that may be null (null's number is 0), a cast whose result
// is then dispatched on, the same test made twice, and a test inside a loop. Each must pass and
// fail the same objects as Java.
public class ClassNumberTests {
    static abstract class Shape {
        abstract int area();
    }

    static class Square extends Shape {
        final int side;

        Square(int side) {
            this.side = side;
        }

        int area() {
            return side * side;
        }
    }

    static class Cube extends Square {
        Cube(int side) {
            super(side);
        }

        int area() {
            return 6 * side * side;
        }
    }

    static class Tile extends Square {
        Tile(int side) {
            super(side);
        }

        int area() {
            return side * side + 1;
        }
    }

    static class Circle extends Shape {
        int area() {
            return 3;
        }
    }

    static class Dot extends Shape {
        int area() {
            return 0;
        }
    }

    static Shape held;

    static Shape make(int i) {
        switch (i % 5) {
            case 0:
                return new Square(i);
            case 1:
                return new Cube(i);
            case 2:
                return new Tile(i);
            case 3:
                return new Circle();
            default:
                return i % 2 == 0 ? new Dot() : null;
        }
    }

    // The range of `Square` and its subclasses, then the dispatch of `area` on the cast value.
    static int squareArea(Shape s) {
        try {
            Square q = (Square) s;
            return q == null ? -1 : q.area();
        } catch (ClassCastException e) {
            return -2;
        }
    }

    // The same test twice on a value that does not change between them.
    static String twice(Shape s) {
        String first = s instanceof Circle ? "circle" : "other";
        return s instanceof Circle ? first + "!" : first + "?";
    }

    public static void main(String[] args) {
        int squares = 0, circles = 0, nulls = 0, total = 0;
        for (int i = 0; i < 20; i++) {
            Shape s = make(i);
            held = s;
            // A static field may be null: one compare tests for null too.
            if (held instanceof Square) squares++;
            if (held instanceof Circle) circles++;
            if (!(held instanceof Shape)) nulls++;
            total += squareArea(s);
            System.out.println(i + " " + (s instanceof Cube) + " " + (s instanceof Tile) + " " + twice(s)
                    + " " + squareArea(s));
        }
        Shape[] all = new Shape[64];
        for (int i = 0; i < all.length; i++) all[i] = make(i);
        int inLoop = 0;
        for (Shape s : all) {
            if (s instanceof Square q) inLoop += q.side;
            else if (s instanceof Dot) inLoop -= 1;
        }
        System.out.println(squares + " " + circles + " " + nulls + " " + total + " " + inLoop);
    }
}
