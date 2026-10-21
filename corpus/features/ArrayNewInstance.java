// `Array.newInstance(a.getClass().getComponentType(), n)`, as the JDK's collections write
// `toArray(T[])`: a new array of `a`'s own type, here of `Point`s and of strings.
import java.lang.reflect.Array;

public class ArrayNewInstance {
    static class Point {
        final int x;

        Point(int x) { this.x = x; }

        public String toString() { return "P" + x; }
    }

    static class Ring<E> {
        private final Object[] items;

        Ring(Object... items) { this.items = items; }

        @SuppressWarnings("unchecked")
        <T> T[] toArray(T[] a) {
            T[] out = a.length >= items.length
                    ? a
                    : (T[]) Array.newInstance(a.getClass().getComponentType(), items.length);
            for (int i = 0; i < items.length; i++) {
                out[i] = (T) items[i];
            }
            if (out.length > items.length) {
                out[items.length] = null;
            }
            return out;
        }
    }

    @SuppressWarnings("unchecked")
    static <T> T[] doubled(T[] a) {
        T[] copy = (T[]) Array.newInstance(a.getClass().getComponentType(), a.length * 2);
        for (int i = 0; i < a.length; i++) {
            copy[2 * i] = a[i];
            copy[2 * i + 1] = a[i];
        }
        return copy;
    }

    public static void main(String[] args) {
        Ring<Point> ring = new Ring<>(new Point(1), new Point(2), new Point(3));
        Point[] grown = ring.toArray(new Point[0]);
        System.out.println(grown.length + " " + grown[0] + " " + grown[2]);
        Point[] room = new Point[4];
        System.out.println((ring.toArray(room) == room) + " " + room[3]);
        Point[] twice = doubled(grown);
        System.out.println(twice.length + " " + twice[1] + " " + twice[5]);
    }
}
