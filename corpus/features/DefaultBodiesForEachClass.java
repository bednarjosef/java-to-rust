// Small default methods called through the dispatcher of an interface above theirs: the arm of
// each class calls a copy of the body for the class, whose calls on `this` run the class's own
// methods: an override in a superclass, an enum constant's method, a record's accessor, another
// default method, a method that throws, and arguments passed on.
public class DefaultBodiesForEachClass {
    interface Sized {
        int size(int scale);
        String label();
    }

    interface Measured extends Sized {
        int base();
        default int size(int scale) { return base() * scale + extra(scale); }
        default int extra(int scale) { return scale > 2 ? base() : 0; }
        default String label() { return name() + ":" + base(); }
        String name();
    }

    static abstract class Shape implements Measured {
        public int base() { return 2; }
    }

    static class Square extends Shape {
        public String name() { return "square"; }
    }

    static final class Big extends Square {
        public int base() { return 10; }
    }

    enum Unit implements Measured {
        METER, MILE {
            public int base() { return 1609; }
        };
        public int base() { return 1; }
    }

    record Box(int side) implements Measured {
        public int base() { return side; }
        public String name() { return "box"; }
    }

    static class Broken implements Measured {
        public int base() { throw new IllegalStateException("no base"); }
        public String name() { return "broken"; }
    }

    static class Plain implements Sized {
        public int size(int scale) { return -scale; }
        public String label() { return "plain"; }
    }

    public static void main(String[] args) {
        Sized[] all = { new Square(), new Big(), Unit.METER, Unit.MILE, new Box(4), new Broken(), new Plain() };
        for (Sized s : all) {
            for (int scale = 1; scale <= 3; scale++) {
                try {
                    System.out.print(s.size(scale) + " ");
                } catch (IllegalStateException e) {
                    System.out.print("(" + e.getMessage() + ") ");
                }
            }
            try {
                System.out.println(s.label());
            } catch (IllegalStateException e) {
                System.out.println("label failed: " + e.getMessage());
            }
        }
    }
}
