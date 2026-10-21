// Objects created from a `Class` value, as `ResourceBundle` creates the locale data that
// `Bundles` finds: `c.getConstructor()`, its modifiers, `setAccessible` and `newInstance`, and
// the older `Class.newInstance()` of a class that `Class.forName` finds by a computed name. The
// candidates are the classes whose names the program spells out; `isAssignableFrom` of a class
// literal tells them apart.
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Modifier;

abstract class Shape {
    abstract String describe();
}

class Circle extends Shape {
    public Circle() {
    }

    String describe() {
        return "circle";
    }
}

class Square extends Shape {
    public Square() {
    }

    String describe() {
        return "square";
    }
}

class Note {
    static int created;

    Note() {
        created++;
    }
}

public class ObjectsFromClassValues {
    static final String[] NAMES = {"Circle", "Square", "Note"};

    static Shape make(Class<? extends Shape> c) {
        try {
            Constructor<? extends Shape> ctor = c.getConstructor();
            if (!Modifier.isPublic(ctor.getModifiers())) {
                return null;
            }
            ctor.setAccessible(true);
            return ctor.newInstance((Object[]) null);
        } catch (NoSuchMethodException | InvocationTargetException | InstantiationException
                 | IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    @SuppressWarnings({"unchecked", "deprecation"})
    public static void main(String[] args) throws Exception {
        for (String name : NAMES) {
            Class<?> c = Class.forName(name);
            boolean shape = Shape.class.isAssignableFrom(c);
            System.out.println(name + " is a shape: " + shape);
            if (shape) {
                System.out.println(make((Class<? extends Shape>) c).describe());
            }
            Object o = c.newInstance();
            System.out.println(o instanceof Shape s ? "made a " + s.describe() : "made something else");
        }
        System.out.println("notes: " + Note.created);
        System.out.println(Shape.class.isAssignableFrom(String.class));
    }
}
