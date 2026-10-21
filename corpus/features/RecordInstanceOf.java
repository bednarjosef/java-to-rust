// instanceof with a record pattern: nested records, components bound without a test, a
// guard in the same condition, the negated form, and pattern variables whose names come back.
public class RecordInstanceOf {
    record Point(int x, int y) {}

    record Segment(Point from, Point to) {}

    sealed interface Shape permits Circle, Square {}

    record Circle(Point center, double radius) implements Shape {}

    record Square(Point corner, double side) implements Shape {}

    static String describe(Object o) {
        if (o instanceof Segment(Point(int fx, int fy), Point to) && fx == 0) {
            return "from the y axis at " + fy + " to " + to;
        }
        if (o instanceof Segment(Point from, Point(var tx, var ty))) {
            return "from " + from + " to (" + tx + ", " + ty + ")";
        }
        if (o instanceof Point(int x, int y)) {
            return "a point at " + (x + y);
        }
        return "something else";
    }

    static double area(Shape s) {
        if (!(s instanceof Circle(Point center, double r))) {
            Square sq = (Square) s;
            return sq.side() * sq.side();
        }
        System.out.println("circle at " + center);
        return 3 * r * r;
    }

    static int sum(Object a, Object b) {
        // The names of the first pattern's variables come back in the second.
        int total = 0;
        if (a instanceof Point(int x, int y)) {
            total += x * y;
        }
        if (b instanceof Point(int y, int x)) {
            total += x - y;
        }
        if (a instanceof Point x) {
            total += x.x();
        }
        return total;
    }

    public static void main(String[] args) {
        Point point = new Point(0, 4);
        System.out.println(describe(new Segment(point, new Point(5, 6))));
        System.out.println(describe(new Segment(new Point(1, 2), new Point(7, 8))));
        System.out.println(describe(new Point(2, 3)));
        System.out.println(describe("text") + " " + describe(null));
        System.out.println(area(new Circle(point, 2)) + " " + area(new Square(point, 3)));
        System.out.println(sum(new Point(3, 4), new Point(10, 1)) + " " + sum("a", null));
        System.out.println(point);
    }
}
