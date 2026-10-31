// Tests of "is an Enum", "is this enum" and "is a Record" over objects of many classes, where
// the enums, records and plain classes are declared interleaved: the enums' classes are numbered
// together (and the records' together), so each test is one range of class numbers.
public class EnumTestsAmongClasses {
    interface Shape {
        double area();
    }

    enum Color {
        RED, GREEN, BLUE
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

    record Point(int x, int y) {}

    enum Planet implements Shape {
        MERCURY(1.0) {
            @Override
            public String toString() {
                return "closest";
            }
        },
        EARTH(3.0);

        final double radius;

        Planet(double radius) {
            this.radius = radius;
        }

        public double area() {
            return radius * radius * 3;
        }
    }

    static class Circle implements Shape {
        final double r;

        Circle(double r) {
            this.r = r;
        }

        public double area() {
            return 3 * r * r;
        }
    }

    record Pair(Object first, Object second) {}

    enum Direction {
        NORTH, SOUTH;

        Direction opposite() {
            return this == NORTH ? SOUTH : NORTH;
        }
    }

    static String describe(Object o) {
        StringBuilder b = new StringBuilder();
        b.append(o instanceof Enum<?> ? "enum " : "");
        b.append(o instanceof Record ? "record " : "");
        b.append(o instanceof Shape ? "shape " : "");
        if (o instanceof Enum<?> e) {
            b.append(e.name()).append('#').append(e.ordinal());
        } else if (o instanceof Record r) {
            b.append(r);
        } else {
            b.append(o.getClass().getSimpleName());
        }
        return b.toString();
    }

    static int ordinalOf(Object o) {
        return ((Enum<?>) o).ordinal();
    }

    public static void main(String[] args) {
        Object[] things = {Color.GREEN, new Square(2), new Point(1, 2), Planet.MERCURY, Planet.EARTH, new Circle(1),
                new Pair(Color.RED, Direction.SOUTH), Direction.NORTH, "text", 7};
        for (Object o : things) {
            System.out.println(describe(o));
        }
        int sum = 0;
        int enums = 0;
        for (int i = 0; i < 1000; i++) {
            Object o = things[i % things.length];
            if (o instanceof Enum<?>) {
                sum += ordinalOf(o);
                enums++;
            }
            if (o instanceof Direction d) {
                sum += 100 * d.opposite().ordinal();
            }
            if (o instanceof Color c) {
                sum += 10 * c.ordinal();
            }
        }
        System.out.println("enums " + enums + ", sum " + sum);
        try {
            System.out.println(ordinalOf(things[1]));
        } catch (ClassCastException e) {
            System.out.println("not an enum");
        }
    }
}
