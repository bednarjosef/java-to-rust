// A covariant array keeps the class it was made of: a `String[]` or a `Shape[]` passed as an
// `Object[]` throws `ArrayStoreException` for an object of another class, and its `getClass()`,
// its clone's and its copies' are its own. A copy into `Object[].class` holds anything, and
// `System.arraycopy` into a narrower array checks each element.
import java.util.Arrays;

public class CovariantArrayClasses {
    interface Shape {
        double area();
    }

    static class Square implements Shape {
        final double side;

        Square(double side) {
            this.side = side;
        }

        public double area() {
            return side * side;
        }

        public String toString() {
            return "Square(" + side + ")";
        }
    }

    static class Circle implements Shape {
        public double area() {
            return 3.0;
        }

        public String toString() {
            return "Circle";
        }
    }

    static String store(Object[] array, int i, Object value) {
        try {
            array[i] = value;
            return "stored " + value;
        } catch (ArrayStoreException e) {
            return "ArrayStoreException: " + e.getMessage();
        } catch (ArrayIndexOutOfBoundsException e) {
            return "ArrayIndexOutOfBoundsException: " + e.getMessage();
        }
    }

    static String copy(Object[] from, Object[] to) {
        try {
            System.arraycopy(from, 0, to, 0, from.length);
            return "copied " + show(to);
        } catch (ArrayStoreException e) {
            return e.getMessage() + " " + show(to);
        }
    }

    static String show(Object[] array) {
        StringBuilder out = new StringBuilder("[");
        for (int i = 0; i < array.length; i++) {
            out.append(i > 0 ? ", " : "").append(array[i]);
        }
        return out.append("]").toString();
    }

    static Object[] same(Object[] array) {
        return array;
    }

    public static void main(String[] args) {
        String[] names = {"ada", "brian"};
        Object[] objects = same(names);
        System.out.println(store(objects, 0, "grace"));
        System.out.println(store(objects, 1, 42));
        System.out.println(store(objects, 2, 3));
        System.out.println(store(objects, 1, null));
        System.out.println(show(names));
        System.out.println(objects.getClass().getSimpleName() + " " + objects.getClass().getName());

        Shape[] shapes = new Shape[2];
        Object[] held = same(shapes);
        System.out.println(store(held, 0, new Square(2)));
        System.out.println(store(held, 1, new Circle()));
        System.out.println(store(held, 1, "circle"));
        System.out.println(held.getClass().getSimpleName());

        Object[] clone = same(names.clone());
        System.out.println(clone.getClass().getSimpleName() + " " + store(clone, 0, 1.5));
        Object[] longer = same(Arrays.copyOf(names, 3));
        System.out.println(longer.getClass().getSimpleName() + " " + store(longer, 2, 'c'));
        Object[] loose = Arrays.copyOf(objects, 2, Object[].class);
        System.out.println(loose.getClass().getSimpleName() + " " + store(loose, 0, 7));

        Object[] plain = {"x", 3, "y"};
        System.out.println(plain.getClass().getSimpleName() + " " + store(plain, 0, new Circle()));
        System.out.println(copy(new Object[] {"p", "q"}, same(new String[2])));
        System.out.println(copy(new Object[] {"p", 4, "r"}, same(new String[3])));
        System.out.println(copy(same(new String[] {"s", "t"}), new Object[2]));
    }
}
