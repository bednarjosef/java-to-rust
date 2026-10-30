// Lookups by name on `Class` values that are not class literals, as gson and log4j make them:
// `o.getClass().getDeclaredFields()` of the objects written out, the fields' values in turn,
// `getSuperclass()` up to `Object`, `getDeclaredFields()` of a class handed in as `Line.class`
// and the fields' types (`getType()`) of those, and `getMethod` of an object's class. Other
// class literals of the program reach no lookup: map keys of a registry, an `EnumSet`'s class,
// a class's name. The translation keeps in its reflection table only the members of the classes
// whose `Class` values a lookup of their kind gets, and the program prints what Java prints.
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.Map;

class Shape {
    String label = "shape";

    public String describe() {
        return "a " + label;
    }
}

class Point extends Shape {
    int x = 1;
    int y = 2;

    public Point() {
        label = "point";
    }
}

class Line extends Shape {
    Point from = new Point();
    Point to = new Point();
    String name = "diagonal";

    public Line() {
        label = "line";
    }

    public String describe() {
        return "a line from " + from.x + " to " + to.y;
    }
}

// Classes whose class literals the program has, but whose `Class` values reach no lookup.
class Unrelated {
    int secret = 7;
    String hidden = "hidden";

    public Unrelated() {
    }

    public String describe() {
        return "unrelated";
    }
}

class Registered {
    long id = 3;

    public Registered() {
    }

    public String describe() {
        return "registered";
    }
}

enum Color {
    RED,
    GREEN
}

public class LookupsOnClassesThatReachThem {
    // Written out as gson writes an object: its fields and those of its superclasses, by name,
    // each value written the same way unless it is a string or a number.
    static String toText(Object o) throws IllegalAccessException {
        if (o instanceof String || o instanceof Number) {
            return String.valueOf(o);
        }
        StringBuilder text = new StringBuilder("{");
        for (Class<?> c = o.getClass(); c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers())) {
                    continue;
                }
                f.setAccessible(true);
                text.append(f.getName()).append('=').append(toText(f.get(o))).append(' ');
            }
        }
        return text.append('}').toString();
    }

    // Filled in as gson fills in an object of a class it is handed: each field of a class of the
    // program filled in the same way, from the field's type, each number field set to `n`.
    static void fill(Class<?> c, Object o, int n) throws ReflectiveOperationException {
        for (Field f : c.getDeclaredFields()) {
            Class<?> type = f.getType();
            f.setAccessible(true);
            if (type == int.class) {
                f.setInt(o, n);
            } else if (type == Point.class) {
                fill(type, f.get(o), n + 1);
            }
        }
    }

    // The method `describe` of an object, found by name on its class.
    static String described(Object o) throws ReflectiveOperationException {
        Method m = o.getClass().getMethod("describe");
        return (String) m.invoke(o);
    }

    public static void main(String[] args) throws Exception {
        Map<Class<?>, Integer> ids = new HashMap<>();
        ids.put(Unrelated.class, 1);
        ids.put(Registered.class, 2);
        ids.put(Shape.class, 3);
        System.out.println("registered as " + ids.get(Registered.class) + ", unrelated as " + ids.get(Unrelated.class));
        System.out.println(Unrelated.class.getSimpleName() + " " + Registered.class.getName());
        System.out.println(EnumSet.allOf(Color.class));
        System.out.println(toText(new Line()));
        Line line = new Line();
        fill(Line.class, line, 5);
        System.out.println(toText(line));
        System.out.println(described(line));
        System.out.println(described(new Point()));
        System.out.println(described(new Unrelated()).length());
    }
}
