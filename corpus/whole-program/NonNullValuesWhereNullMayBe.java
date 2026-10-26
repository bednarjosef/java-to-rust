// Values that are not null where the value may be: an assignment to an element of a variable
// that a null test narrowed, returned where the method may return null (fastutil's
// `computeIfPresent`, `return value[pos] = newValue`); an enum constant's `name()` called
// through an interface that another class answers with null (`ProtocolFamily.name()`); and a
// lambda's call of a method that never returns null, where another lambda of its interface may
// (Minecraft's `var0 -> AllayAi.getActivities()`).
public class NonNullValuesWhereNullMayBe {
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

    interface Provider<E> {
        java.util.List<String> activities(E e);
    }

    static java.util.List<String> one() {
        return java.util.List.of("a");
    }

    static java.util.List<String> maybe(int n) {
        return n > 2 ? null : java.util.List.of("b");
    }

    static final Provider<Integer> FIRST = var0 -> one();
    static final Provider<Integer> SECOND = var0 -> maybe(var0);

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
        System.out.println(FIRST.activities(1) + " " + SECOND.activities(args.length + 3) + " " + SECOND.activities(0));
    }
}
