import java.util.Arrays;

// Arrays.copyOf on arrays of references: strings, objects and a class's objects, longer (padded
// with null) and shorter, as ArrayList grows its Object[] backing array.
public class ArraysCopyOfReferences {
    static class Point {
        final int x;

        Point(int x) {
            this.x = x;
        }

        public String toString() {
            return "P" + x;
        }
    }

    static Object[] elements = new Object[2];
    static int size = 0;

    static void add(Object o) {
        if (size == elements.length) {
            elements = Arrays.copyOf(elements, elements.length * 2);
        }
        elements[size++] = o;
    }

    public static void main(String[] args) {
        String[] words = { "a", "b", "c" };
        String[] longer = Arrays.copyOf(words, 5);
        System.out.println(longer.length + " " + longer[2] + " " + longer[4]);
        String[] shorter = Arrays.copyOf(words, 2);
        shorter[0] = "z";
        System.out.println(shorter.length + " " + shorter[0] + " " + words[0]);

        Point[] points = { new Point(1), new Point(2) };
        Point[] more = Arrays.copyOf(points, 3);
        System.out.println(more[1] + " " + more[2] + " " + (more[0] == points[0]));

        for (int i = 0; i < 5; i++) {
            add("item" + i);
        }
        System.out.println(size + " " + elements.length + " " + elements[4] + " " + elements[5]);
    }
}
