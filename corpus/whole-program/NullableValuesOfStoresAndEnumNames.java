// Two values that may be null where the value at hand is not: an assignment to an element of a
// variable that a null test narrowed, returned where the method may return null (fastutil's
// `computeIfPresent`, `return value[pos] = newValue`), and an enum constant's `name()` called
// through an interface that another class answers with null (`ProtocolFamily.name()`).
public class NullableValuesOfStoresAndEnumNames {
    interface Named {
        String name();
    }

    enum Family implements Named { INET, UNIX }

    static final class Nameless implements Named {
        public String name() {
            return null;
        }
    }

    static final class Map {
        Object[] values = new String[3];

        Object store(int i, Object v) {
            if (v == null) {
                return values.length > 10 ? null : "none";
            }
            return values[i] = v;
        }
    }

    static String nameOf(Named n) {
        return n.name();
    }

    public static void main(String[] args) {
        Map map = new Map();
        System.out.println(map.store(1, "x") + " " + map.store(2, null) + " " + map.values[1]);
        map.values = new Object[3];
        System.out.println(map.store(0, 7) + " " + map.values[0]);
        Named[] all = {Family.INET, new Nameless(), Family.UNIX};
        for (Named n : all) {
            System.out.println(nameOf(n));
        }
    }
}
