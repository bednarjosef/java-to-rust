import java.util.function.IntSupplier;

// Casts, tests and calls on a local whose class an earlier test already proved, where the
// translation does not test the class again; and where the local changes in between, where it
// must.
public class ProvenClassTests {
    interface Shape { int area(); String name(); }

    static abstract class Round implements Shape {
        final int r;
        Round(int r) { this.r = r; }
        public String name() { return "round"; }
    }

    static final class Circle extends Round {
        Circle(int r) { super(r); }
        public int area() { return 3 * r * r; }
        int radius() { return r; }
        public String name() { return "circle"; }
    }

    static final class Ring extends Round {
        final int inner;
        Ring(int r, int inner) { super(r); this.inner = inner; }
        public int area() { return 3 * (r * r - inner * inner); }
    }

    static final class Square implements Shape {
        final int side;
        Square(int side) { this.side = side; }
        public int area() { return side * side; }
        public String name() { return "square"; }
        int side() { return side; }
    }

    static final class Node {
        final int value;
        final Object next;
        Node(int value, Object next) { this.value = value; this.next = next; }
    }

    static int total(Object[] things) {
        int sum = 0;
        for (int i = 0; i < things.length; i++) {
            Object o = things[i];
            if (o instanceof Circle) {
                Circle c = (Circle) o;
                sum += c.radius() + ((Shape) o).area() + ((Circle) o).area();
                // Proven: a `Circle` is a `Shape`, and never a `Square`.
                if (o instanceof Shape) sum += 1;
                if (o instanceof Square) sum += 1000;
            } else if (o instanceof Round) {
                sum += ((Round) o).area() + ((Round) o).r;
                // Among a `Circle` and a `Ring`, the test of a `Circle` is left.
                if (o instanceof Circle) sum += 1000;
                if (o instanceof Round r) sum += r.r;
            }
            if (!(o instanceof Shape)) continue;
            Shape s = (Shape) o;
            sum += s.area() + ((Shape) o).name().length();
            sum += o instanceof Square && ((Square) o).side() > 3 ? ((Square) o).side() : 0;
            sum += o instanceof Square ? ((Square) o).side() : -1;
        }
        return sum;
    }

    // `o` changes inside the region of the test: the casts after the change test again.
    static String changing(Object[] things) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < things.length; i++) {
            Object o = things[i];
            if (o instanceof Circle) {
                out.append(((Circle) o).radius()).append(' ');
                o = things[(i + 1) % things.length];
                try {
                    out.append(((Circle) o).radius()).append(' ');
                } catch (ClassCastException e) {
                    out.append("not a circle ");
                }
            }
            Object p = things[i];
            if (p instanceof Shape) {
                for (int j = 0; j < 3; j++) {
                    try {
                        out.append(((Shape) p).area()).append(',');
                    } catch (ClassCastException e) {
                        out.append("changed,");
                    }
                    if (j == 1) p = "text";
                }
                out.append(' ');
            }
            Object q = things[i];
            if (q instanceof Square) {
                try {
                    q = things[i + 1];
                } catch (ArrayIndexOutOfBoundsException e) {
                    out.append("last ");
                }
                try {
                    out.append(((Square) q).side()).append(' ');
                } catch (ClassCastException e) {
                    out.append("not a square ");
                }
            }
        }
        return out.toString();
    }

    // A test in a loop's condition, and the local the loop moves along.
    static int chain(Object start) {
        int sum = 0;
        Object at = start;
        while (at instanceof Node) {
            Node n = (Node) at;
            sum += n.value + ((Node) at).value;
            at = n.next;
        }
        return sum + (at == null ? 0 : ((Shape) at).area());
    }

    // A test seen by a lambda that captures the local, by the branch after an `else` that leaves,
    // and a cast that may be of null.
    static int captured(Object[] things) {
        int sum = 0;
        for (int i = 0; i < things.length; i++) {
            Object o = things[i];
            if (o instanceof Circle) {
                IntSupplier f = () -> ((Circle) o).radius() + ((Shape) o).area();
                sum += f.getAsInt();
            }
            if (o instanceof Round) {
                sum += 1;
            } else {
                continue;
            }
            sum += ((Round) o).r + ((Shape) o).area();
            Object maybe = i % 2 == 0 ? null : o;
            if (maybe == null || maybe instanceof Circle) {
                Circle c = (Circle) maybe;
                Shape shape = (Shape) maybe;
                sum += c == null ? 7 : c.radius();
                sum += shape == null ? 11 : shape.name().length();
            }
        }
        return sum;
    }

    // A `switch` whose cases fall through, one of them changing the local.
    static int cases(Object[] things, int k) {
        int sum = 0;
        Object o = things[0];
        if (o instanceof Circle) {
            switch (k) {
                case 0:
                    sum += ((Circle) o).radius();
                case 1:
                    o = things[2];
                case 2:
                    try {
                        sum += ((Circle) o).radius() * 10;
                    } catch (ClassCastException e) {
                        sum += 100;
                    }
                    break;
                default:
                    sum += ((Shape) o).area();
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        Object[] things = {
            new Circle(2), new Ring(3, 1), new Square(4), "text", new Circle(1), new Square(2), new Ring(5, 2)
        };
        long sum = 0;
        for (int round = 0; round < 3; round++) {
            sum += total(things);
        }
        System.out.println(sum);
        System.out.println(changing(things));
        Object list = new Node(1, new Node(2, new Node(3, new Square(3))));
        System.out.println(chain(list) + " " + chain(new Node(4, null)));
        System.out.println(captured(things));
        for (int k = 0; k < 4; k++) {
            System.out.print(cases(things, k) + " ");
        }
        System.out.println();
    }
}
