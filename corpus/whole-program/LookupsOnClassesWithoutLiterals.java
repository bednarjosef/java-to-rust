// Lookups on the `Class` values of classes that the program names in no class literal: the class
// of an object (`getClass()`) and its superclasses (`getSuperclass()`), whose declared fields and
// methods are listed and read, as gson writes an object out. The translation finds these classes
// by following where `Class` values go, so that their members are in its reflection table, and
// the program prints what Java prints.
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

abstract class Shape {
    String label;
    int corners;

    Shape(String label, int corners) {
        this.label = label;
        this.corners = corners;
    }

    public String kind() {
        return "shape";
    }
}

class Square extends Shape {
    double side;

    Square(double side) {
        super("square", 4);
        this.side = side;
    }

    public String kind() {
        return "square";
    }

    public double area() {
        return side * side;
    }
}

class Circle extends Shape {
    double radius;

    Circle(double radius) {
        super("circle", 0);
        this.radius = radius;
    }
}

public class LookupsOnClassesWithoutLiterals {
    // Each declared field that is not static, of the object's class and its superclasses below
    // `Object`, sorted by name within a class, as name=value.
    static String fields(Object o) throws IllegalAccessException {
        StringBuilder text = new StringBuilder(o.getClass().getSimpleName()).append(" {");
        for (Class<?> c = o.getClass(); c != Object.class; c = c.getSuperclass()) {
            Field[] declared = c.getDeclaredFields();
            java.util.Arrays.sort(declared, (a, b) -> a.getName().compareTo(b.getName()));
            for (Field f : declared) {
                if (Modifier.isStatic(f.getModifiers())) {
                    continue;
                }
                f.setAccessible(true);
                text.append(' ').append(c.getSimpleName()).append('.').append(f.getName()).append('=').append(f.get(o));
            }
        }
        return text.append(" }").toString();
    }

    // The public methods that the object's superclass declares, by name.
    static String inherited(Object o) {
        Method[] declared = o.getClass().getSuperclass().getDeclaredMethods();
        java.util.Arrays.sort(declared, (a, b) -> a.getName().compareTo(b.getName()));
        StringBuilder text = new StringBuilder();
        for (Method m : declared) {
            if (Modifier.isPublic(m.getModifiers())) {
                text.append(m.getName()).append(' ');
            }
        }
        return text.toString().trim();
    }

    public static void main(String[] args) throws Exception {
        Shape[] shapes = { new Square(2.5), new Circle(1.0) };
        for (Shape s : shapes) {
            System.out.println(fields(s));
            System.out.println(s.getClass().getSuperclass().getSimpleName() + " declares " + inherited(s));
            System.out.println(s.getClass().getDeclaredMethods().length + " methods in " + s.kind());
        }
    }
}
