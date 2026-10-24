// A covariant array's class is its own wherever Java looks at it: `String[].class` is not
// `Object[].class`, `getClass()` of a `String[]` held as an `Object[]` or an `Object` is
// `String[].class`, and `instanceof` and casts to an array class test the class the array was
// made of, as do an enum's `values()` and the classes in a `Class[]`.
public class ArrayElementClasses {
    enum Day {
        MON,
        TUE
    }

    interface Named {}

    static class Thing implements Named {}

    static Object[] same(Object[] a) {
        return a;
    }

    static String kind(Object o) {
        String s = "";
        s += (o instanceof String[]) ? "S" : "-";
        s += (o instanceof Integer[]) ? "I" : "-";
        s += (o instanceof Object[]) ? "O" : "-";
        if (o instanceof String[] strings) {
            s += " strings " + strings.length;
        }
        try {
            String[] t = (String[]) o;
            s += " cast ok " + t.length;
        } catch (ClassCastException e) {
            s += " " + e.getMessage();
        }
        return s;
    }

    static String narrow(Object[] objects) {
        try {
            Integer[] numbers = (Integer[]) objects;
            return "Integer[] of " + numbers.length;
        } catch (ClassCastException e) {
            return e.getMessage();
        }
    }

    public static void main(String[] args) {
        String[] names = {"a", "b"};
        Object[] objects = same(names);
        Class<?>[] classes = {String.class};
        Object[] cs = same(classes);
        Integer[] ints = {1, 2};
        Object[] io = same(ints);
        System.out.println("1 " + (objects.getClass() == String[].class));
        System.out.println("2 " + (objects.getClass() == Object[].class));
        System.out.println("3 " + (String[].class == (Class<?>) Object[].class));
        System.out.println("4 " + (cs.getClass() == Class[].class));
        System.out.println("5 " + (objects.getClass() == (Class<?>) Class[].class));
        System.out.println("6 " + String[].class.getName() + " " + Class[].class.getSimpleName());
        System.out.println("7 " + kind(objects));
        System.out.println("8 " + kind(io));
        System.out.println("9 " + kind("text"));
        Object held = objects;
        System.out.println("10 " + held.getClass().getSimpleName() + " " + held.getClass().getName());
        System.out.println("11 " + narrow(io) + " | " + narrow(objects));
        Object[] days = same(Day.values());
        System.out.println("12 " + days.getClass().getSimpleName() + " " + days.getClass().getComponentType().isEnum());
        Object[] interfaces = same(Thing.class.getInterfaces());
        System.out.println("13 " + interfaces.getClass().getSimpleName() + " " + interfaces.length);
        System.out.println("14 " + io.getClass().getComponentType().getName());
    }
}
