// Loops that search an array for a key and return where they find it, which the translation
// compares four elements to a turn: found at each place of a turn and in the elements left after
// the last whole turn, not found, from a start other than zero, up to a bound other than the
// length, by identity among objects, strings and enum constants held as `Object`s, and the
// exceptions of a null array and of indexes out of bounds, in the order Java throws them.
public class ArraySearchLoops {
    enum Shape { ROUND, SQUARE, FLAT }

    static Object[] palette;

    int size;
    long[] stamps = new long[6];

    static int idFor(Object key) {
        for (int i = 0; i < palette.length; i++) {
            if (palette[i] == key) {
                return i;
            }
        }
        return -1;
    }

    static int indexOf(int[] values, int wanted, int from) {
        for (int i = from; i < values.length; i++) {
            if (values[i] == wanted) {
                return i;
            }
        }
        return -1;
    }

    static boolean anyAbove(double[] values, double limit) {
        for (int i = 0; i < values.length; i++) {
            if (values[i] > limit && values[i] != 99.5) {
                return true;
            }
        }
        return false;
    }

    static String firstAbove(int[] values, int limit) {
        for (int i = 0; i < values.length; i++) {
            if (values[i] > limit) {
                return "at " + i + " is " + values[i];
            }
        }
        return "none above " + limit;
    }

    // Up to a bound of its own, which may pass the array's end.
    int stampIndex(long stamp) {
        for (int i = 0; i < size; i++) {
            if (stamps[i] == stamp) {
                return i;
            }
        }
        return -1;
    }

    static int firstOf(char[] text, char wanted) {
        for (int i = 0; i < text.length; i++) {
            if (wanted == text[i]) {
                return i;
            }
        }
        return -1;
    }

    static int countFrom(Shape[] shapes, Shape wanted, int from) {
        for (int i = from; i < shapes.length; i++) {
            if (shapes[i] == wanted) {
                return i - from;
            }
        }
        return -1;
    }

    public static void main(String[] args) {
        Shape[] shapes = Shape.values();
        palette = new Object[11];
        palette[0] = "air";
        palette[1] = "stone";
        for (int i = 2; i < palette.length; i++) {
            palette[i] = shapes[i % 3];
        }
        for (Shape shape : shapes) {
            System.out.println(shape + " at " + idFor(shape));
        }
        // A string key, which is not compared as bits: the strings of the palette are other strings.
        System.out.println("dirt at " + idFor("dirt"));
        Object[] wide = new Object[palette.length];
        for (int i = 0; i < wide.length; i++) {
            wide[i] = new Object();
        }
        palette = wide;
        for (int k = 0; k < wide.length; k++) {
            System.out.println("object " + k + " at " + idFor(wide[k]));
        }
        System.out.println("missing object at " + idFor(new Object()));
        System.out.println("null at " + idFor(null));
        palette[7] = null;
        System.out.println("null now at " + idFor(null));
        palette = new Object[0];
        System.out.println("empty palette: " + idFor(Shape.ROUND));

        int[] values = new int[13];
        for (int i = 0; i < values.length; i++) {
            values[i] = (i * 7) % 13;
        }
        for (int wanted = 0; wanted < 14; wanted++) {
            System.out.println(wanted + " from 0: " + indexOf(values, wanted, 0) + ", from 5: " + indexOf(values, wanted, 5));
        }
        System.out.println("from past the end: " + indexOf(values, 3, 20));
        try {
            System.out.println(indexOf(values, 3, -2));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("negative start: " + e.getMessage());
        }
        try {
            System.out.println(indexOf(null, 3, 0));
        } catch (NullPointerException e) {
            System.out.println("null array throws");
        }
        System.out.println(anyAbove(new double[] { 1.5, 99.5, 2.5 }, 50) + " " + anyAbove(new double[] { 1, 2, 3, 4, 5, 60 }, 50));
        System.out.println(firstAbove(values, 10) + "; " + firstAbove(values, 12));

        ArraySearchLoops stamps = new ArraySearchLoops();
        for (int i = 0; i < stamps.stamps.length; i++) {
            stamps.stamps[i] = 1000L * i;
        }
        stamps.size = 4;
        System.out.println("stamp 3000 at " + stamps.stampIndex(3000) + ", 5000 at " + stamps.stampIndex(5000));
        stamps.size = 9;
        System.out.println("stamp 5000 at " + stamps.stampIndex(5000));
        try {
            System.out.println(stamps.stampIndex(7));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past the end: " + e.getMessage());
        }

        char[] text = "a search through some text".toCharArray();
        System.out.println(firstOf(text, 's') + " " + firstOf(text, 'x') + " " + firstOf(text, 't'));

        Shape[] row = { Shape.FLAT, Shape.ROUND, Shape.SQUARE, Shape.FLAT, Shape.ROUND };
        System.out.println(countFrom(row, Shape.SQUARE, 0) + " " + countFrom(row, Shape.ROUND, 2) + " " + countFrom(row, Shape.SQUARE, 3));
    }
}
