// Small default methods called through the dispatcher of an interface above theirs: the arm of
// each class calls a copy of the body for the class, whose calls on `this` run the class's own
// methods: an override in a superclass, an enum constant's method, a record's accessor, another
// default method, a method that throws, and arguments passed on, one of them named `o`, one a
// cast to String that may be null (a bridge such as `add(Object)` calling `add(String)`).
// Four classes run the bodies, as many as are copied for.
public class DefaultBodiesForEachClass {
    interface Sized {
        int size(int scale);
        String label();
        boolean has(Object o);
        boolean hasName(Object o);
    }

    interface Measured extends Sized {
        int base();
        default int size(int scale) { return base() * scale + extra(scale); }
        default int extra(int scale) { return scale > 2 ? base() : 0; }
        default String label() { return name() + ":" + base(); }
        String name();
        default boolean has(Object o) { return indexOf(o) >= 0; }
        int indexOf(Object o);
        default boolean hasName(Object o) { return isName((String) o); }
        default boolean isName(String s) { return s != null && s.equals(name()); }
    }

    static abstract class Shape implements Measured {
        public int base() { return 2; }
        public int indexOf(Object o) { return name().equals(o) ? 0 : -1; }
    }

    static class Square extends Shape {
        public String name() { return "square"; }
    }

    enum Unit implements Measured {
        METER, MILE {
            public int base() { return 1609; }
        };
        public int base() { return 1; }
        public int indexOf(Object o) { return name().equals(o) ? 1 : -1; }
    }

    record Box(int side) implements Measured {
        public int base() { return side; }
        public String name() { return "box"; }
        public int indexOf(Object o) { return name().equals(o) ? 2 : -1; }
    }

    static class Broken implements Measured {
        public int base() { throw new IllegalStateException("no base"); }
        public String name() { return "broken"; }
        public int indexOf(Object o) { return -1; }
    }

    static class Plain implements Sized {
        public int size(int scale) { return -scale; }
        public String label() { return "plain"; }
        public boolean has(Object o) { return "plain".equals(o); }
        public boolean hasName(Object o) { return o == null; }
    }

    public static void main(String[] args) {
        Sized[] all = { new Square(), Unit.METER, Unit.MILE, new Box(4), new Broken(), new Plain() };
        for (Sized s : all) {
            for (int scale = 1; scale <= 3; scale++) {
                try {
                    System.out.print(s.size(scale) + " ");
                } catch (IllegalStateException e) {
                    System.out.print("(" + e.getMessage() + ") ");
                }
            }
            System.out.print(s.has("box") + " " + s.has("square") + " ");
            System.out.print(s.hasName(null) + " " + s.hasName("box") + " " + s.hasName("METER") + " ");
            try {
                System.out.println(s.label());
            } catch (IllegalStateException e) {
                System.out.println("label failed: " + e.getMessage());
            }
        }
    }
}
