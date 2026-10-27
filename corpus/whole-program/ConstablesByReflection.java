import java.lang.constant.Constable;
import java.lang.reflect.Method;

// A program whose objects may be classes (a class held as a `Type` makes them so), with a method
// that takes a `Constable`, which its reflection may look up, and `Constable`'s own
// `describeConstable`. The table's call of either would treat an object as a `Constable`, which
// a class held as an object cannot be in a translation, so the table leaves them out. The names
// looked up at run time are others.
public class ConstablesByReflection {
    public static class Shapes {
        public static String describe(Constable c) {
            return "constable " + c;
        }

        public static int size(Object o) {
            return o.toString().length();
        }
    }

    static String find(String name, Object argument) {
        try {
            Method m = switch (name) {
                case "describe" -> Shapes.class.getMethod("describe", Constable.class);
                case "describeConstable" -> Constable.class.getMethod("describeConstable");
                default -> Shapes.class.getMethod(name, Object.class);
            };
            return m.getName() + " " + m.invoke(null, argument);
        } catch (Exception e) {
            return e.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        java.lang.reflect.Type type = String.class;
        Object[] values = {type, "text", 3, int[].class};
        for (Object v : values) {
            System.out.println((v instanceof Comparable) + " " + (v instanceof Class) + " " + find("size", v));
        }
        System.out.println(Shapes.describe("direct"));
        String[] names = {"describe", "describeConstable", "size", "length"};
        int first = args.length > 3 ? 0 : 2;
        for (int i = first; i < names.length; i++) {
            System.out.println(find(names[i], values[i]));
        }
    }
}
