import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.stream.Stream;

/**
 * Every way code can see an array's class, on arrays that the translation holds as `Object[]`s:
 * each array type here meets `Object[]` once, so each is one everywhere, and what Java says of
 * its class must still be its own.
 */
public class CovariantArrayClasses {
    enum Day {
        MON,
        TUE
    }

    interface Shape {}

    static final class Point implements Shape {
        final int x;

        Point(int x) {
            this.x = x;
        }

        @Override
        public String toString() {
            return "Point" + x;
        }
    }

    static String[] names = {"b", "a", "c"};
    static Integer[] numbers = {3, 1, 2};
    static Point[] points = {new Point(1), new Point(2)};
    static Shape[] shapes = {new Point(3)};
    static Day[] days = Day.values();
    static String[][] grid = {{"x", "y"}, {"z"}};

    static String[] echo(String[] in) {
        return in;
    }

    static int count(String... items) {
        System.out.println("varargs " + items.getClass().getName());
        return items.length;
    }

    static String before(Object o) {
        String text = String.valueOf(o);
        return text.substring(0, text.indexOf('@') + 1);
    }

    static void describe(String label, Object o) {
        Class<?> c = o.getClass();
        System.out.println(label + ": " + c.getName() + " " + c.getSimpleName() + " array=" + c.isArray()
                + " component=" + c.getComponentType().getName() + " text=" + before(o)
                + " objects=" + (o instanceof Object[]) + " strings=" + (o instanceof String[])
                + " integers=" + (o instanceof Integer[]) + " shapes=" + (o instanceof Shape[])
                + " points=" + (o instanceof Point[]) + " grid=" + (o instanceof String[][])
                + " length=" + Array.getLength(o));
    }

    /** What Java says of the class of a plain `Object[]`, which tests for narrower classes leave out. */
    static void describeClass(String label, Object o) {
        Class<?> c = o.getClass();
        System.out.println(label + ": " + c.getName() + " " + c.getSimpleName() + " component="
                + c.getComponentType().getName() + " text=" + before(o) + " objects=" + (o instanceof Object[])
                + " length=" + Array.getLength(o));
    }

    static void store(String label, Object[] array, Object value) {
        try {
            array[0] = value;
            System.out.println(label + ": stored " + array[0]);
        } catch (ArrayStoreException e) {
            System.out.println(label + ": ArrayStoreException");
        }
    }

    static void cast(String label, Object o) {
        try {
            String[] s = (String[]) o;
            System.out.println(label + ": cast to String[] of " + s.length);
        } catch (ClassCastException e) {
            System.out.println(label + ": ClassCastException");
        }
    }

    public static void main(String[] args) throws Exception {
        // The meetings that make each array type an `Object[]` in the translation.
        Object[][] all = {names, numbers, points, shapes, days, grid, grid[0]};
        System.out.println("arrays " + all.length);

        describe("names", names);
        describe("numbers", numbers);
        describe("points", points);
        describe("shapes", shapes);
        describe("days", days);
        describe("grid", grid);
        describe("row", grid[1]);
        describe("new String[2]", new String[2]);
        describe("new String[2][2]", new String[2][2]);
        describe("row of new String[2][2]", (new String[2][2])[0]);
        describeClass("object", new Object[] {"a"});

        System.out.println("literal " + (String[].class == names.getClass()) + " " + String[].class.getName()
                + " " + ((Object) Object[].class == names.getClass()) + " " + String[][].class.getName());
        System.out.println("assignable " + Object[].class.isAssignableFrom(String[].class) + " "
                + String[].class.isAssignableFrom(Object[].class) + " "
                + Shape[].class.isAssignableFrom(points.getClass()));
        System.out.println("element " + ((Object[]) names)[0].getClass().getName());

        cast("names", names);
        cast("numbers", numbers);

        store("names with Integer", names.clone(), 5);
        store("names with String", names.clone(), "s");
        store("points with Point", points.clone(), new Point(9));
        store("shapes with Point", shapes.clone(), new Point(8));
        store("points with String", points.clone(), "p");
        store("days with String", days.clone(), "d");
        store("object with Integer", new Object[] {"a"}, 5);

        describe("clone", names.clone());
        describe("copyOf", Arrays.copyOf(names, 5));
        describe("copyOfRange", Arrays.copyOfRange(numbers, 1, 3));
        describeClass("copyOf to Object[]", Arrays.copyOf(names, 2, Object[].class));
        describe("copyOf to Shape[]", Arrays.copyOf(points, 1, Shape[].class));
        describeClass("asList toArray", Arrays.asList(names).toArray());
        describe("stream toArray", Stream.of("p", "q").toArray(String[]::new));
        describeClass("stream toArray plain", Stream.of("p", "q").toArray());
        describe("Array.newInstance", Array.newInstance(String.class, 3));
        
        describe("echo", echo(names));
        List<String> list = new ArrayList<>(List.of("m", "n"));
        String[] filled = new String[3];
        describe("toArray filled", list.toArray(filled));
        System.out.println("filled same " + (list.toArray(filled) == filled) + " " + filled[2]);

        Object[] target = new Object[3];
        System.arraycopy(names, 0, target, 0, 3);
        describeClass("arraycopy into Object[]", target);
        String[] into = new String[3];
        System.arraycopy(new Object[] {"u", "v"}, 0, into, 0, 2);
        System.out.println("arraycopy into String[] " + Arrays.toString(into));
        try {
            System.arraycopy(new Object[] {"u", 7}, 0, into, 0, 2);
            System.out.println("arraycopy wrong stored");
        } catch (ArrayStoreException e) {
            System.out.println("arraycopy wrong ArrayStoreException " + into[0]);
        }

        System.out.println("count " + count("a", "b") + " " + count(names));

        String[] sorted = names.clone();
        Arrays.sort(sorted);
        Integer[] sortedNumbers = numbers.clone();
        Arrays.sort(sortedNumbers, Collections.reverseOrder());
        String[] fill = new String[2];
        Arrays.fill(fill, "f");
        System.out.println("sorted " + Arrays.toString(sorted) + " " + Arrays.toString(sortedNumbers) + " "
                + Arrays.toString(fill));
        System.out.println("equals " + Arrays.equals(names, new String[] {"b", "a", "c"}) + " "
                + Arrays.hashCode(names) + " " + Arrays.deepEquals(grid, new String[][] {{"x", "y"}, {"z"}})
                + " " + String.join("+", names) + " " + Arrays.asList(days));

        for (Method m : CovariantArrayClasses.class.getDeclaredMethods()) {
            if (m.getName().equals("echo")) {
                System.out.println("echo method " + m.getParameterTypes()[0].getName() + " "
                        + m.getReturnType().getName() + " " + m.getReturnType().getComponentType().getName());
                describe("invoked", m.invoke(null, (Object) names));
            }
        }
        Field field = CovariantArrayClasses.class.getDeclaredField("points");
        System.out.println("field " + field.getType().getName() + " " + field.getType().getSimpleName());
        describe("field get", field.get(null));
        Field gridField = CovariantArrayClasses.class.getDeclaredField("grid");
        System.out.println("grid field " + gridField.getType().getName());
    }
}
