// Arrays of classes held as `Object[]` (an array of classes is passed where an `Object[]` is
// expected), and the arrays of classes that reflection makes: `getMethod`'s parameter types,
// `getInterfaces()`.
import java.lang.reflect.Method;
import java.util.Arrays;

public class ClassArraysAsObjectArrays {
    interface Named {
        String name();
    }

    static class Item implements Named, Comparable<Item> {
        public String name() {
            return "item";
        }

        public int compareTo(Item other) {
            return 0;
        }
    }

    public static String twice(String s, int n) {
        return s.repeat(n);
    }

    static int count(Object[] values) {
        return values.length;
    }

    public static void main(String[] args) throws Exception {
        Class<?>[] types = {String.class, int.class};
        System.out.println(count(types) + " " + Arrays.toString(types));
        Method m = ClassArraysAsObjectArrays.class.getMethod("twice", String.class, int.class);
        System.out.println(m.getName() + " " + m.getParameterCount() + " " + m.invoke(null, "ab", 2));
        Class<?>[] interfaces = Item.class.getInterfaces();
        System.out.println(count(interfaces) + " " + interfaces[0].getSimpleName() + " " + interfaces[1].getSimpleName());
    }
}
