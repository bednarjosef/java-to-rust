// Features: records with a compact canonical constructor that validates, extra methods and static
// factories on records, equals/hashCode/toString of records, records as HashMap keys, a sealed
// interface permitting records, a switch expression with type patterns and guards (when), record
// deconstruction patterns, nested records, and instanceof with a record pattern.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class RecordsAndSealed {
    record Point(int x, int y) {
        Point {
            if (x < -1000 || y < -1000) {
                throw new IllegalArgumentException("coordinate out of range: " + x + "," + y);
            }
        }

        static Point origin() {
            return new Point(0, 0);
        }

        Point plus(Point other) {
            return new Point(x + other.x, y + other.y);
        }

        int manhattan() {
            return Math.abs(x) + Math.abs(y);
        }
    }

    record Segment(Point from, Point to) {
        int length() {
            return Math.abs(to.x() - from.x()) + Math.abs(to.y() - from.y());
        }
    }

    record Named<T>(String name, T value) {
    }

    sealed interface Shape permits Circle, Rect, Group {
    }

    record Circle(Point center, int radius) implements Shape {
    }

    record Rect(Point corner, int width, int height) implements Shape {
    }

    record Group(String label, List<Shape> members) implements Shape {
    }

    static int area(Shape shape) {
        return switch (shape) {
            case Circle c when c.radius() == 0 -> 0;
            case Circle c -> 3 * c.radius() * c.radius();
            case Rect(Point p, int w, int h) -> w * h;
            case Group g -> {
                int total = 0;
                for (Shape s : g.members()) {
                    total += area(s);
                }
                yield total;
            }
        };
    }

    static String classify(Object o) {
        return switch (o) {
            case null -> "null value";
            case Integer i when i < 0 -> "negative int " + i;
            case Integer i -> "int " + i;
            case String s -> "string of length " + s.length();
            case Point(int x, int y) when x == y -> "diagonal point " + x;
            case Point p -> "point " + p;
            default -> "something else: " + o.getClass().getSimpleName();
        };
    }

    public static void main(String[] args) {
        Point a = new Point(3, -4);
        Point b = new Point(3, -4);
        System.out.println(a + " equals " + b + ": " + a.equals(b) + ", same hash: " + (a.hashCode() == b.hashCode()));
        System.out.println("plus: " + a.plus(Point.origin()).plus(new Point(1, 1)));
        System.out.println("manhattan: " + a.manhattan());

        Segment seg = new Segment(Point.origin(), a);
        System.out.println("segment: " + seg + " length " + seg.length());

        Map<Point, String> labels = new HashMap<>();
        labels.put(a, "a");
        labels.put(new Point(0, 0), "origin");
        System.out.println("lookup equal record: " + labels.get(new Point(3, -4)));
        System.out.println("labels: " + labels.size());

        try {
            new Point(-2000, 0);
        } catch (IllegalArgumentException e) {
            System.out.println("rejected: " + e.getMessage());
        }

        Named<List<Integer>> named = new Named<>("primes", List.of(2, 3, 5));
        System.out.println(named.name() + " has " + named.value().size() + " values: " + named.value());

        List<Shape> members = new ArrayList<>();
        members.add(new Circle(Point.origin(), 2));
        members.add(new Rect(new Point(1, 1), 3, 4));
        Group inner = new Group("inner", List.of(new Circle(a, 0), new Rect(a, 2, 2)));
        members.add(inner);
        Shape all = new Group("all", members);
        for (Shape s : members) {
            System.out.println("area of " + s.getClass().getSimpleName() + " = " + area(s));
        }
        System.out.println("total area = " + area(all));

        if (seg instanceof Segment(Point(int fx, int fy), Point to) && fx == 0) {
            System.out.println("segment starts on the y axis? " + (fy == 0) + " ends at " + to);
        }

        Object[] things = {null, -5, 12, "hello", new Point(7, 7), new Point(1, 2), 2.5};
        for (Object thing : things) {
            System.out.println(classify(thing));
        }
    }
}
