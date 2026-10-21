// Classes as values: kept in variables and fields, compared, printed, and asked for their names,
// superclass and component type.
public class ClassValues {
    static class Animal {}

    static class Dog extends Animal {}

    interface Named {}

    enum Color { RED, GREEN }

    static Class<?> kept;

    final Class<? extends Animal> kind;

    ClassValues(Animal a) {
        kind = a.getClass();
    }

    static Class<?> comparableClass(Object x) {
        Class<?> c;
        if ((c = x.getClass()) == String.class) {
            return c;
        }
        return null;
    }

    static String describe(Class<?> c) {
        return c.getName() + " " + c.getSimpleName() + " " + c.isArray() + " " + c.isInterface() + " " + c.isPrimitive();
    }

    public static void main(String[] args) {
        Animal a = new Dog();
        Class<?> c = a.getClass();
        System.out.println(c + " " + (c == Dog.class) + " " + (c != Animal.class));
        System.out.println(c.getSuperclass() + " " + c.getSuperclass().getSuperclass() + " " + Object.class.getSuperclass());
        System.out.println(describe(int[].class) + " / " + describe(String[][].class) + " / " + describe(Named.class));
        System.out.println(int[].class.getComponentType() + " " + String.class.getComponentType());
        System.out.println(describe(int.class) + " / " + describe(Color.class) + " " + Color.class.isEnum());
        System.out.println(Color.RED.getClass() == Color.class);
        System.out.println(comparableClass("text") + " " + comparableClass(3));
        Object o = "s";
        System.out.println(o.getClass().getName() + " " + Integer.valueOf(4).getClass().getSimpleName());
        kept = new ClassValues(new Animal()).kind;
        System.out.println(kept + " " + kept.equals(Animal.class) + " " + kept.equals(null) + " " + (kept == null));
        Class<?> none = null;
        System.out.println(none + " " + (none == kept) + " " + Object[].class.getName());
    }
}
