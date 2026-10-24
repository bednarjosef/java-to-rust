// `java.lang.Record` as a type: values of it holding the program's records, `equals`, `hashCode`
// and `toString` through it, casts to and from it, `instanceof Record`, and the intersection
// `Record & Shape` that a conditional over two records of a sealed interface has, and a null
// `Record`.
public class RecordValues {
    sealed interface Shape permits Circle, Square {}

    record Circle(int radius) implements Shape {}

    record Square(int side) implements Shape {
        @Override
        public String toString() {
            return "Square of " + side;
        }
    }

    record Pair(String left, Integer right) {}

    static final class Plain {
        @Override
        public String toString() {
            return "plain";
        }
    }

    static String describe(Record r) {
        return r + " " + r.hashCode() + " " + r.equals(new Circle(2));
    }

    static Shape pick(boolean round) {
        return (Shape) (round ? new Circle(1) : new Square(3));
    }

    static <T> T either(boolean first, T a, T b) {
        return first ? a : b;
    }

    static Shape shape(Object o) {
        return (Shape) (Record) o;
    }

    public static void main(String[] args) {
        Record[] records = {new Circle(2), new Square(4), new Pair("a", 1), new Pair(null, null)};
        for (Record r : records) {
            System.out.println(describe(r));
        }
        Record same = new Pair("a", 1);
        System.out.println(same.equals(records[2]) + " " + records[2].equals(same) + " " + same.equals(null));
        System.out.println(same.equals("a") + " " + (same.hashCode() == records[2].hashCode()));
        System.out.println(pick(true) + " " + pick(false));
        var chosen = either(false, new Circle(6), new Square(7));
        Shape asShape = chosen;
        Record asRecord = chosen;
        System.out.println(chosen + " " + asShape.equals(asRecord) + " " + chosen.hashCode());
        Object[] objects = {new Circle(5), "text", new Plain(), 7, new Square(1)};
        for (Object o : objects) {
            if (o instanceof Record r) {
                System.out.println("record " + r);
            } else {
                System.out.println("not a record " + o);
            }
        }
        System.out.println(shape(new Square(9)));
        try {
            shape(new Pair("b", 2));
        } catch (ClassCastException e) {
            System.out.println("not a shape");
        }
        try {
            Record r = (Record) objects[1];
            System.out.println(r);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }
        Object back = records[0];
        Circle c = (Circle) (Record) back;
        System.out.println(c.radius() + " " + back.getClass().getSuperclass().getName());
        Record none = args.length > 5 ? same : null;
        try {
            System.out.println(none.hashCode());
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            System.out.println(none.equals(same));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        System.out.println(none + " " + (none instanceof Record) + " " + ((Record) (Object) none == null));
    }
}
