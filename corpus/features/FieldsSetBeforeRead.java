// Fields that every constructor sets, to a value that is not null, before anything can read them,
// though not as the constructor's first statements: their reads need no null test. Next to them,
// fields that may be null, and fields read before they are set, which stay checked.
public class FieldsSetBeforeRead {
    static int made = 0;

    static void check(boolean ok, String what) {
        if (!ok) {
            throw new IllegalArgumentException(what);
        }
    }

    // Set after a call, in both branches of an `if` (the parameter narrowed by its test), and by
    // a constructor that delegates.
    static final class Storage {
        final int size;
        final long[] data;
        final String label;

        Storage(int size) {
            this(size, null);
        }

        Storage(int size, long[] data) {
            check(size > 0, "size");
            this.size = size;
            if (data != null) {
                if (data.length != size) {
                    throw new IllegalArgumentException("length " + data.length);
                }
                this.data = data;
            } else {
                this.data = new long[size];
            }
            this.label = "storage of " + size;
        }

        Storage(int[] values) {
            this(values.length);
            for (int i = 0; i < values.length; i++) {
                data[i] = values[i];
            }
        }

        long sum() {
            long total = 0;
            for (long v : data) {
                total += v;
            }
            return total;
        }
    }

    // An object that passes `this` on, after it has set the field: the constructor allocates the
    // object first.
    static final class Node {
        final String name;
        final Node[] children;
        int childCount;
        Node parent;

        Node(String name, Node parent) {
            this.name = name;
            this.children = new Node[4];
            made++;
            this.parent = parent;
            if (parent != null) {
                parent.children[parent.childCount++] = this;
            }
        }

        int depth() {
            return parent == null ? 0 : 1 + parent.depth();
        }

        int count() {
            int n = 1;
            for (int i = 0; i < childCount; i++) {
                n += children[i].count();
            }
            return n;
        }
    }

    // A field reassigned later, always with values that are not null.
    static class Counter {
        StringBuilder log;
        Object last;
        int ticks;

        Counter() {
            ticks = 0;
            log = new StringBuilder("start");
            last = log;
        }

        void tick(String what) {
            ticks++;
            log.append(' ').append(what);
            if (ticks % 3 == 0) {
                log = new StringBuilder(log.length() > 20 ? "reset" : log.toString());
            }
            last = what;
        }
    }

    // Read through a subclass, whose constructor runs the superclass's first.
    static class Shape {
        final String kind;
        final double[] dims;

        Shape(String kind, double... dims) {
            check(dims.length > 0, kind);
            this.kind = kind;
            this.dims = dims.clone();
        }

        double area() {
            return dims[0] * dims[0];
        }

        String describe() {
            return kind + " " + area();
        }
    }

    static final class Rect extends Shape {
        final String note;

        Rect(double w, double h) {
            super("rect", w, h);
            this.note = kind + "!";
        }

        @Override
        double area() {
            return dims[0] * dims[1];
        }
    }

    // A field that a superclass's constructor reads through an override, before the subclass's
    // constructor sets it: Java finds it null, so it stays checked.
    static class Base {
        final String seen;

        Base() {
            seen = String.valueOf(peek());
        }

        Object peek() {
            return null;
        }
    }

    static final class Late extends Base {
        final StringBuilder late;

        Late() {
            late = new StringBuilder("late");
        }

        @Override
        Object peek() {
            return late;
        }
    }

    // A field read by a method that the constructor calls before setting it.
    static final class Early {
        String value;
        final int length;

        Early(String v) {
            length = measure();
            value = v;
        }

        int measure() {
            return value == null ? -1 : value.length();
        }
    }

    // A constructor that may return before it sets the field, and one that may set it to null.
    static final class Maybe {
        String text;
        String other;

        Maybe(boolean early, String other) {
            this.other = other;
            if (early) {
                return;
            }
            text = "set";
        }
    }

    // Set inside a `try`, whose `catch` sets it too, and after a loop that does not touch it.
    static final class Parsed {
        final Integer number;
        final String source;
        final int[] digits;

        Parsed(String s) {
            int count = 0;
            for (int i = 0; i < s.length(); i++) {
                if (Character.isDigit(s.charAt(i))) {
                    count++;
                }
            }
            this.digits = new int[count];
            Integer n;
            try {
                n = Integer.valueOf(s.trim());
            } catch (NumberFormatException e) {
                n = -1;
            }
            this.number = n;
            this.source = s;
        }
    }

    // Arrays that every constructor sets after a check, read in a loop.
    static final class Grid {
        final int width;
        final int[] cells;
        final int[] weights;

        Grid(int width, int height) {
            check(width > 0 && height > 0, "grid");
            this.width = width;
            this.cells = new int[width * height];
            this.weights = new int[width];
            for (int i = 0; i < weights.length; i++) {
                weights[i] = i + 1;
            }
        }

        long weigh(int rounds) {
            long total = 0;
            for (int r = 0; r < rounds; r++) {
                for (int i = 0; i < cells.length; i++) {
                    cells[i] += weights[i % width];
                    total += cells[i];
                }
            }
            return total;
        }
    }

    // Set by the constructor, and to null later: a read after that throws as Java's does.
    static final class Holder {
        String text;

        Holder(String text) {
            check(text != null, "text");
            this.text = text;
        }

        void clear() {
            text = null;
        }

        int length() {
            return text.length();
        }
    }

    // Read by a method that the constructor calls before it sets the field: Java throws.
    static final class Eager {
        final String value;
        final int length;

        Eager(String v) {
            length = measure();
            value = v;
        }

        int measure() {
            return value.length();
        }
    }

    public static void main(String[] args) {
        Storage a = new Storage(4);
        Storage b = new Storage(new int[] {3, 1, 4, 1, 5});
        Storage c = new Storage(3, new long[] {7, 8, 9});
        System.out.println(a.label + " " + a.sum() + " " + a.data.length);
        System.out.println(b.label + " " + b.sum());
        System.out.println(c.label + " " + c.sum());
        try {
            new Storage(2, new long[] {1});
        } catch (IllegalArgumentException e) {
            System.out.println("refused " + e.getMessage());
        }
        try {
            new Storage(0);
        } catch (IllegalArgumentException e) {
            System.out.println("refused " + e.getMessage());
        }

        Node root = new Node("root", null);
        Node x = new Node("x", root);
        Node y = new Node("y", x);
        new Node("z", root);
        System.out.println(root.count() + " " + y.depth() + " " + y.parent.name + " " + made);
        y.parent = root;
        System.out.println(y.depth() + " " + root.childCount + " " + x.children[0].name);

        Counter counter = new Counter();
        for (String w : "a bb ccc dddd eeeee ffffff g".split(" ")) {
            counter.tick(w);
        }
        System.out.println(counter.log + " " + counter.last + " " + counter.ticks);

        Shape[] shapes = {new Shape("square", 3), new Rect(2, 5)};
        for (Shape s : shapes) {
            System.out.println(s.describe() + " " + s.dims.length);
        }
        System.out.println(((Rect) shapes[1]).note);

        Late late = new Late();
        System.out.println(late.seen + " " + late.late);
        Early early = new Early("four");
        System.out.println(early.length + " " + early.value);
        Maybe m1 = new Maybe(true, null);
        Maybe m2 = new Maybe(false, "o");
        System.out.println(m1.text + " " + m2.text + " " + m1.other + " " + m2.other);

        Grid grid = new Grid(7, 5);
        System.out.println("grid " + grid.weigh(200) + " " + grid.cells[34]);
        Holder holder = new Holder("abc");
        System.out.println("holder " + holder.length());
        holder.clear();
        try {
            System.out.println(holder.length());
        } catch (NullPointerException e) {
            System.out.println("holder " + e.getMessage());
        }
        try {
            System.out.println(new Eager("x").length);
        } catch (NullPointerException e) {
            System.out.println("eager " + e.getMessage());
        }

        for (String s : new String[] {"42", " 7 ", "x1y2"}) {
            Parsed p = new Parsed(s);
            System.out.println(p.number + " " + p.source.trim() + " " + p.digits.length);
        }
    }
}
