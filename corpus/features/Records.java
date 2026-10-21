// Records: canonical, compact and custom constructors, accessors, the generated `toString`,
// `equals` and `hashCode`, static members, a record implementing an interface, a generic record
// and records nested in records.
public class Records {
    interface Shape {
        double area();

        String name();
    }

    record Point(int x, int y) {
        static final Point ORIGIN = new Point(0, 0);
        static int made;

        Point {
            made++;
        }

        Point(int both) {
            this(both, both);
        }

        static Point of(int x, int y) {
            return new Point(x, y);
        }

        Point plus(Point other) {
            return new Point(x + other.x, y() + other.y());
        }

        int distance() {
            return Math.abs(x) + Math.abs(y);
        }
    }

    record Range(int low, int high) {
        Range {
            if (low > high) {
                int t = low;
                low = high;
                high = t;
            }
        }

        int length() {
            return high - low;
        }
    }

    record Circle(String name, double radius) implements Shape {
        public double area() {
            return Math.PI * radius * radius;
        }
    }

    record Square(String name, double side) implements Shape {
        public double area() {
            return side * side;
        }

        @Override
        public String toString() {
            return "Square of side " + side;
        }
    }

    record Line(Point from, Point to) {
        int length() {
            return Math.abs(to.x() - from.x()) + Math.abs(to.y() - from.y());
        }
    }

    record Box<T>(T value, int count) {
        T first() {
            return value;
        }
    }

    record Mixed(long id, boolean on, char tag, float weight, short small, byte tiny, String label) {}

    record Empty() {}

    enum Color { RED, GREEN }

    record Pixel(Point at, Color color) {}

    static void describe(Shape s) {
        System.out.printf("%s %.3f%n", s.name(), s.area());
    }

    public static void main(String[] args) {
        Point p = new Point(3, -4);
        Point q = Point.of(3, -4);
        System.out.println(p + " " + p.x() + " " + p.y() + " " + p.distance());
        System.out.println(p.equals(q) + " " + (p == q) + " " + p.hashCode() + " " + q.hashCode());
        System.out.println(p.equals(Point.ORIGIN) + " " + Point.ORIGIN + " " + Point.ORIGIN.hashCode());
        System.out.println(p.plus(new Point(7)) + " made " + Point.made);

        Range r = new Range(9, 2);
        System.out.println(r + " length " + r.length() + " " + r.equals(new Range(2, 9)));

        Shape[] shapes = {new Circle("circle", 1.5), new Square("square", 2)};
        for (Shape s : shapes) {
            describe(s);
            System.out.println(s);
        }
        Circle c = new Circle("c", 0.5);
        System.out.println(c.equals(new Circle("c", 0.5)) + " " + c.equals(new Circle("d", 0.5)));
        System.out.println(c.hashCode() == new Circle("c", 0.5).hashCode());
        Circle nameless = new Circle(null, 2);
        System.out.println(nameless + " " + nameless.equals(new Circle(null, 2)) + " " + nameless.hashCode());
        System.out.println(nameless.equals(c) + " " + c.equals(nameless));

        Line line = new Line(new Point(1, 1), new Point(4, 5));
        System.out.println(line + " " + line.length() + " " + line.from().x());
        System.out.println(line.equals(new Line(new Point(1, 1), new Point(4, 5))) + " " + line.hashCode());

        Box<String> box = new Box<>("hello", 2);
        System.out.println(box + " " + box.first().length() + " " + box.value() + " " + box.count());
        Box<Point> points = new Box<>(p, 1);
        System.out.println(points.first().x() + " " + points);

        Mixed m = new Mixed(1L << 40, true, 'z', 2.5f, (short) -3, (byte) 7, "label");
        System.out.println(m);
        System.out.println(m.hashCode() + " " + m.equals(new Mixed(1L << 40, true, 'z', 2.5f, (short) -3, (byte) 7, "label")));
        System.out.println(m.id() + " " + m.on() + " " + m.tag() + " " + m.weight() + " " + m.small() + " " + m.tiny());

        Empty e = new Empty();
        System.out.println(e + " " + e.hashCode() + " " + e.equals(new Empty()));

        Pixel px = new Pixel(new Point(2, 3), Color.GREEN);
        System.out.println(px + " " + px.color() + " " + px.equals(new Pixel(new Point(2, 3), Color.GREEN)));
        System.out.println(px.equals(new Pixel(new Point(2, 3), Color.RED)));
    }
}
