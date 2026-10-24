import java.lang.reflect.Method;

// `Method.isDefault` and `isSynthetic` of the reflection table's methods: an interface's default
// method, its abstract and static ones, a class's own and inherited ones.
public class ReflectionDefaultMethods {
    interface Shape {
        default int sides() {
            return 0;
        }

        int area();

        static Shape unit() {
            return null;
        }
    }

    static class Square implements Shape {
        public int area() {
            return 1;
        }
    }

    static void show(Class<?> c, String name) throws Exception {
        Method m = c.getMethod(name);
        System.out.println(c.getSimpleName() + "." + name + " default=" + m.isDefault() + " synthetic="
                + m.isSynthetic());
    }

    public static void main(String[] args) throws Exception {
        show(Shape.class, "sides");
        show(Shape.class, "area");
        show(Shape.class, "unit");
        show(Square.class, "area");
        show(Square.class, "sides");
    }
}
