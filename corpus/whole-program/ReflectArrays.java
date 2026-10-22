import java.lang.reflect.Array;
import java.util.Arrays;

// `java.lang.reflect.Array` on a component type known only as a `Class` value, as javac's
// `AnnotationProxyMaker` builds an annotation's array value: `newInstance` of the component type
// of the element's method's return type, filled with `set`, and its failures in the JVM's order.
public class ReflectArrays {
    enum Level { LOW, HIGH }

    static Object fill(Class<?> arrayClass, Object... values) {
        Object array = Array.newInstance(arrayClass.getComponentType(), values.length);
        for (int i = 0; i < values.length; i++) {
            Array.set(array, i, values[i]);
        }
        return array;
    }

    static void attempt(String what, Runnable action) {
        try {
            action.run();
            System.out.println(what + ": stored");
        } catch (IllegalArgumentException | ArrayIndexOutOfBoundsException | NullPointerException
                | NegativeArraySizeException e) {
            System.out.println(what + ": " + e);
        }
    }

    public static void main(String[] args) {
        String[] names = (String[]) fill(String[].class, "a", "b", null);
        System.out.println(Arrays.toString(names) + " " + Array.getLength(names));
        int[] numbers = (int[]) fill(int[].class, 1, (short) 2, 'c');
        System.out.println(Arrays.toString(numbers));
        long[] longs = (long[]) fill(long[].class, 5L, 6);
        double[] doubles = (double[]) fill(double[].class, 1.5, 2.5f, 3L);
        System.out.println(Arrays.toString(longs) + " " + Arrays.toString(doubles));
        Level[] levels = (Level[]) fill(Level[].class, Level.HIGH, Level.LOW);
        System.out.println(Arrays.toString(levels));
        Object[] objects = (Object[]) fill(Object[].class, "x", 7, Level.LOW);
        System.out.println(Arrays.toString(objects));
        boolean[] flags = (boolean[]) fill(boolean[].class, true, false);
        System.out.println(Arrays.toString(flags));

        Object target = new int[2];
        attempt("int from a string", () -> Array.set(target, 0, "x"));
        attempt("int from null", () -> Array.set(target, 0, null));
        attempt("int out of range", () -> Array.set(target, 5, "x"));
        attempt("char from an int", () -> Array.set(new char[1], 0, 3));
        attempt("string from an int", () -> Array.set(names, 0, 3));
        attempt("string out of range", () -> Array.set(names, -1, 3));
        attempt("not an array", () -> Array.set("text", 0, 3));
        attempt("null array", () -> Array.set(null, 0, 3));
        attempt("length of text", () -> Array.getLength("text"));
        attempt("negative length", () -> Array.newInstance(String.class, -1));
        attempt("void elements", () -> Array.newInstance(void.class, 1));
        attempt("null component", () -> Array.newInstance(null, 1));
    }
}
