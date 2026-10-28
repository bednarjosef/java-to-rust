import java.lang.reflect.Array;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;

/**
 * Arrays whose class the program learns only at run time, as log4j's configuration makes them:
 * `options.toArray(EMPTY)` copies a list into an array of the given array's class, which a
 * reflective call then checks against its parameter's `String[]`; and `Array.newInstance` of a
 * plugin's element class, filled through an `Object[]` and passed to a factory method.
 */
public class RuntimeArrayClasses {
    interface Policy {
        String name();
    }

    static class Daily implements Policy {
        public String name() {
            return "daily";
        }
    }

    static class Sized implements Policy {
        public String name() {
            return "sized";
        }
    }

    static final String[] EMPTY = {};

    public static String join(String[] options) {
        return String.join(",", options);
    }

    public static String policies(Policy[] policies) {
        StringBuilder out = new StringBuilder();
        for (Policy p : policies) {
            out.append(p.name()).append(' ');
        }
        return out.toString().trim();
    }

    static Object[] elements(Class<?> type, List<?> values) {
        Object[] array = (Object[]) Array.newInstance(type, values.size());
        for (int i = 0; i < array.length; i++) {
            array[i] = values.get(i);
        }
        return array;
    }

    public static void main(String[] args) throws Exception {
        List<String> options = new ArrayList<>(List.of("HH:mm", "UTC"));
        String[] copied = options.toArray(EMPTY);
        System.out.println(copied.getClass().getName() + " " + copied.length);
        Method join = RuntimeArrayClasses.class.getMethod("join", String[].class);
        System.out.println(join.invoke(null, (Object) copied));
        System.out.println(join.invoke(null, (Object) new ArrayList<String>().toArray(EMPTY)));

        Method policies = RuntimeArrayClasses.class.getMethod("policies", Policy[].class);
        Class<?> element = policies.getParameterTypes()[0].getComponentType();
        Object[] made = elements(element, List.of(new Daily(), new Sized()));
        System.out.println(made.getClass().getSimpleName() + " " + made.length);
        System.out.println(policies.invoke(null, (Object) made));
        try {
            made[0] = "not a policy";
        } catch (ArrayStoreException e) {
            System.out.println("ArrayStoreException " + e.getMessage());
        }
    }
}
