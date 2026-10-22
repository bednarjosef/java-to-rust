public class NullableBooleanCondition {
    static String pick(Boolean recurse, String name) {
        if (recurse) {
            return name + "!";
        } else {
            return name;
        }
    }

    static Boolean maybe(int a) {
        return a < 0 ? null : a == 1;
    }

    static String value(Object value) {
        String s = (((Boolean) value) ? "1L" : "0L");
        return s;
    }

    public static void main(String[] args) {
        System.out.println(pick(true, "a"));
        System.out.println(pick(false, "b"));
        try {
            System.out.println(pick(null, "c"));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        Boolean flag = maybe(-1);
        try {
            while (flag) {
                System.out.println("looping");
            }
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        flag = maybe(1);
        System.out.println(flag ? "yes" : "no");
        System.out.println(value(Boolean.TRUE));
        System.out.println(value(Boolean.FALSE));
        try {
            System.out.println(value(null));
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        pick(maybe(-2), "d");
    }
}
