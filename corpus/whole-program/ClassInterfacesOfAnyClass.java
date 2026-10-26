import java.io.Serializable;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Supplier;

// `Class.getInterfaces()` of a class that is no class literal, as gson's `getGenericSupertype`
// walks a class's interfaces and superclasses to find the `Collection` or `Map` a field's type
// implements: classes of the program, the JDK's collections, the runtime's own classes, arrays,
// enums and lambdas, with interfaces the translation leaves out (`Serializable`, `Cloneable`).
public class ClassInterfacesOfAnyClass {
    interface Shape {}

    interface Named extends Shape, Comparable<Named> {}

    static class Square implements Named, Serializable, Cloneable {
        public int compareTo(Named other) {
            return 0;
        }
    }

    static class Big extends Square implements Runnable {
        public void run() {}
    }

    enum Color implements Shape { RED }

    static String names(Class<?> c) {
        StringBuilder out = new StringBuilder();
        for (Class<?> i : c.getInterfaces()) {
            out.append(out.length() == 0 ? "" : ",").append(i.getSimpleName());
            if (!i.isInterface()) {
                out.append("(not an interface)");
            }
        }
        return c.getSimpleName() + ": [" + out + "]";
    }

    // gson's search for the supertype `target` of `raw`, through interfaces then superclasses.
    static boolean implementsSomewhere(Class<?> raw, Class<?> target) {
        if (raw == target) {
            return true;
        }
        for (Class<?> i : raw.getInterfaces()) {
            if (implementsSomewhere(i, target)) {
                return true;
            }
        }
        Class<?> superclass = raw.getSuperclass();
        return superclass != null && implementsSomewhere(superclass, target);
    }

    public static void main(String[] args) {
        Supplier<String> lambda = () -> "x";
        List<Class<?>> classes = List.of(
                Square.class, Big.class, Named.class, Shape.class, Color.class,
                ArrayList.class, HashMap.class, String.class, Integer.class, Object.class,
                int.class, int[].class, Square[].class, new Big().getClass());
        for (Class<?> c : classes) {
            System.out.println(names(c));
        }
        System.out.println(lambda.getClass().getInterfaces().length + " "
                + lambda.getClass().getInterfaces()[0].getSimpleName());
        System.out.println(implementsSomewhere(ArrayList.class, java.util.Collection.class) + " "
                + implementsSomewhere(Big.class, Shape.class) + " "
                + implementsSomewhere(Big.class, Map.class));
        Class<?>[] interfaces = Big.class.getSuperclass().getInterfaces();
        interfaces[0] = null;
        System.out.println(Arrays.toString(interfaces).length() > 0 && Square.class.getInterfaces()[0] == Named.class);
        Class<?> none = classes.size() > 100 ? Object.class : null;
        try {
            none.getInterfaces();
        } catch (NullPointerException e) {
            System.out.println("null class " + e.getMessage());
        }
    }
}
