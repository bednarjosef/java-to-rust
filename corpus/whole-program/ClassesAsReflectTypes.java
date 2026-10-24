import java.lang.reflect.Type;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

// Classes held as `java.lang.reflect.Type`s, beside a type of the program's own that is one:
// `instanceof Class` with and without a binding, casts back (one that fails), `getTypeName()`,
// `equals`, `hashCode`, `toString` and `==` through the interface, as gson's and guava's type
// tokens use them.
public class ClassesAsReflectTypes {
    record Named(String name) implements Type {
        public String getTypeName() {
            return "named " + name;
        }
    }

    static final class Plain implements Type {
        public String toString() {
            return "plain";
        }
    }

    static class Inner {}

    static Type pick(int i) {
        return switch (i) {
            case 0 -> String.class;
            case 1 -> new Named("x");
            case 2 -> int[].class;
            case 3 -> null;
            case 4 -> Inner.class;
            case 5 -> String[][].class;
            case 6 -> new Plain();
            case 7 -> int.class;
            default -> Runnable.class;
        };
    }

    static String describe(Type type) {
        if (type instanceof Class<?> c) {
            return "class " + c.getSimpleName() + " " + c.isArray() + " " + c.isInterface();
        }
        return type == null ? "null" : "other " + type.getTypeName();
    }

    static Class<?> raw(Type type) {
        if (type instanceof Class) {
            return (Class<?>) type;
        }
        if (type instanceof Named n) {
            return n.name().length() == 1 ? Object.class : null;
        }
        throw new IllegalArgumentException("not a class: " + type);
    }

    public static void main(String[] args) {
        for (int i = 0; i < 9; i++) {
            Type type = pick(i);
            System.out.println(describe(type) + " " + (type instanceof Class) + " " + (type instanceof Named));
            if (type instanceof Class && ((Class<?>) type).isArray()) {
                System.out.println("array of " + ((Class<?>) type).getComponentType().getName());
            }
            if (type != null) {
                System.out.println(type.getTypeName() + " / " + type + " / " + type.toString());
                System.out.println(type.equals(String.class) + " " + type.equals(pick(i)) + " " + (type == pick(i)));
                System.out.println((type.hashCode() == pick(i).hashCode()) + " " + (type == String.class));
            }
        }
        System.out.println(String.class.getTypeName() + " " + long[][].class.getTypeName() + " " + Inner.class.getTypeName());
        List<Type> types = new ArrayList<>();
        types.add(Integer.class);
        types.add(new Named("list"));
        types.add(boolean[].class);
        for (Type t : types) {
            System.out.println(t.getTypeName() + " " + raw(t));
        }
        System.out.println(types.contains(Integer.class) + " " + types.indexOf(boolean[].class) + " " + types.contains(Long.class));
        Type[] array = {String.class, int.class, new Named("in an array")};
        for (Type t : array) {
            System.out.println(describe(t));
        }
        Map<Type, String> byType = new HashMap<>();
        byType.put(Integer.class, "a box");
        byType.put(String.class, "text");
        byType.put(new Named("key"), "named");
        System.out.println(byType.get(String.class) + " " + byType.get(Long.class) + " " + byType.size());
        System.out.println(Objects.equals(array[0], String.class) + " " + String.valueOf(array[1]));
        Type named = pick(1);
        try {
            Class<?> bad = (Class<?>) named;
            System.out.println(bad);
        } catch (ClassCastException e) {
            System.out.println("cast failed");
        }
        try {
            raw(pick(6));
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        // A class's type name is its interned name, an array's a new string.
        System.out.println((String.class.getTypeName() == "java.lang.String") + " "
                + (pick(0).getTypeName() == "java.lang.String") + " " + (pick(2).getTypeName() == "int[]") + " "
                + (pick(1).getTypeName() == "named x"));
        Object o = pick(0);
        System.out.println((o instanceof Type) + " " + ((Type) o).getTypeName() + " " + (o instanceof Class));
    }
}
