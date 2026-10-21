// The identity shortcut before `equals` on `Object`s that may be strings or boxes, as the JDK's
// maps write it: `a == b || a.equals(b)`, and with the key read into a variable on the way,
// `(k = keys[i]) == key || (key != null && key.equals(k))`, also as a later operand of a chain of
// `||`: `!matchValue || (v = value) == expected || (expected != null && expected.equals(v))`.
public class EqualsShortcut {
    static Object[] keys = {"a", 2, "c", null};

    static int find(Object key) {
        for (int i = 0; i < keys.length; i++) {
            Object k;
            if ((k = keys[i]) == key || (key != null && key.equals(k))) {
                return i;
            }
        }
        return -1;
    }

    static boolean same(Object a, Object b) {
        return a == b || a != null && a.equals(b);
    }

    static boolean matches(boolean matchValue, int i, Object expected) {
        Object v;
        return !matchValue || (v = keys[i]) == expected || (expected != null && expected.equals(v));
    }

    public static void main(String[] args) {
        System.out.println(find("c") + " " + find("ab".substring(0, 1)) + " " + find(2) + " " + find(null)
                + " " + find("z"));
        System.out.println(matches(false, 0, "q") + " " + matches(true, 0, "ab".substring(0, 1)) + " "
                + matches(true, 1, 3) + " " + matches(true, 3, null));
        System.out.println(same("x", "x") + " " + same(1, 2) + " " + same(null, null) + " " + same(null, 3));
    }
}
