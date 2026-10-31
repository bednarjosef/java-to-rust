import java.util.EnumMap;

// `getClass()` of values held as `Object`, of every kind a program holds there: enum constants
// with and without a body, own classes and records, strings, boxes, arrays of primitives, arrays
// whose class is fixed and `Object[]`s that are arrays of a subtype. Each class is read from a
// table by the class's number, except an `Object[]` that may be a covariant array, which carries
// its class. `EnumMap`'s test of a key: its class is the key type, or the key type is its
// superclass; and `EnumMap` itself, given keys of every kind.
public class ClassesOfObjectsByNumber {
    enum Plain { A, B }

    enum WithBodies {
        UP { int sign() { return 1; } },
        DOWN { int sign() { return -1; } },
        FLAT;

        int sign() { return 0; }
    }

    static class Animal { }

    static final class Dog extends Animal { }

    record Point(int x, int y) { }

    static boolean validKey(Object key, Class<?> keyType) {
        if (key == null) {
            return false;
        }
        Class<?> keyClass = key.getClass();
        return keyClass == keyType || keyClass.getSuperclass() == keyType;
    }

    public static void main(String[] args) {
        Object[] covariant = new String[] { "s" };
        Object[] values = {
            Plain.A, Plain.B, WithBodies.UP, WithBodies.DOWN, WithBodies.FLAT, new Animal(), new Dog(),
            new Point(1, 2), "text", 3, 4L, 'c', true, new int[1], new double[2][2], new Dog[1],
            covariant, new Object[] { 1 }, new Object(),
        };
        for (Object value : values) {
            Class<?> c = value.getClass();
            // A constant with a body is of a class of its own, which the translation gives the
            // enum's name: the enum's names are left out.
            String name = value instanceof WithBodies ? "" : c.getName() + " " + c.getSimpleName();
            System.out.println(name + " plain " + validKey(value, Plain.class) + " bodies "
                + validKey(value, WithBodies.class) + " same " + (c == values[0].getClass()));
        }
        System.out.println(validKey(null, Plain.class));
        System.out.println(WithBodies.UP.getDeclaringClass() == WithBodies.FLAT.getClass());
        System.out.println(covariant.getClass() == String[].class);
        EnumMap<WithBodies, String> map = new EnumMap<>(WithBodies.class);
        map.put(WithBodies.UP, "up");
        map.put(WithBodies.FLAT, null);
        for (Object value : values) {
            System.out.println(map.get(value) + " " + map.containsKey(value));
        }
    }
}
