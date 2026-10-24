import java.util.Iterator;

// Methods that return a new object of their own (factories), called where the object stays in the
// caller, so that the caller's slot holds it; and calls whose objects must stay in the heap.
public class StackFactories {
    static class Cursor {
        int at;
        final int end;
        Cursor(int start, int end) { at = start; this.end = end; }
        boolean more() { return at < end; }
        int next() { return at++; }
    }

    static class Range implements Iterable<Integer> {
        final int start, end;
        Range(int start, int end) { this.start = start; this.end = end; }
        // A factory.
        Cursor cursor() { return new Cursor(start, end); }
        // A factory that passes on another's object.
        Cursor again() { return cursor(); }
        // Not a factory: it may return an object made earlier.
        Cursor shared() { return start == 0 ? ZERO : new Cursor(start, end); }
        public Iterator<Integer> iterator() {
            return new Iterator<Integer>() {
                int at = start;
                public boolean hasNext() { return at < end; }
                public Integer next() { return at++; }
            };
        }
    }

    static final Cursor ZERO = new Cursor(0, 3);

    // A static factory.
    static Cursor upTo(int end) { return new Cursor(0, end); }

    // Shapes with an override each, so calls on them dispatch.
    static abstract class Shape {
        abstract Cursor corners();
    }
    static class Square extends Shape {
        Cursor corners() { return new Cursor(0, 4); }
    }
    static class Triangle extends Shape {
        Cursor corners() { return new Cursor(0, 3); }
    }

    static Cursor kept;

    static int sum(Cursor c) {
        int total = 0;
        while (c.more()) total += c.next();
        return total;
    }

    public static void main(String[] args) {
        Range r = new Range(2, 7);
        long total = 0;
        for (int i = 0; i < 1000; i++) {
            total += sum(r.cursor());
            total += sum(r.again());
            total += sum(upTo(i % 10));
            for (int x : r) total += x;
            Cursor s = new Range(i % 2, 5).shared();
            total += sum(s);
        }
        System.out.println(total);
        // The zero cursor was used up once and stays used.
        System.out.println(ZERO.more());

        // A cursor kept past its loop's iteration stays in the heap.
        Cursor previous = null;
        for (int i = 0; i < 5; i++) {
            Cursor c = r.cursor();
            if (previous != null) total += previous.at;
            previous = c;
            c.next();
        }
        System.out.println(total + " " + previous.at);

        // One stored in a static stays in the heap.
        kept = r.again();
        kept.next();
        System.out.println(kept.at);

        // Calls through an abstract method.
        Shape[] shapes = { new Square(), new Triangle() };
        int corners = 0;
        for (int round = 0; round < 100; round++) {
            for (Shape shape : shapes) corners += sum(shape.corners());
        }
        System.out.println(corners);

        // A collection cycle while objects are in slots.
        long garbage = 0;
        for (int round = 0; round < 2000; round++) {
            Cursor c = r.cursor();
            int[] noise = new int[64];
            noise[round % 64] = round;
            garbage += noise[round % 64] + sum(c);
            Integer[] boxes = new Integer[8];
            for (int k = 0; k < boxes.length; k++) boxes[k] = k * round;
            garbage += boxes[7];
        }
        System.out.println(garbage);
    }
}
