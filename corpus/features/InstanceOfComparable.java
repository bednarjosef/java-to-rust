public class InstanceOfComparable {
    enum Size { SMALL, LARGE }

    // Comparable values of a bounded type variable, told apart by their classes.
    static <T extends Comparable<T>> String describe(T value) {
        if (value instanceof Integer i) return "int " + (i + 1);
        if (value instanceof String s) return "text " + s.length();
        if (value instanceof Double) return "double " + value;
        if (value instanceof Size size) return "size " + size.ordinal();
        return "other " + value;
    }

    static String kind(Comparable<?> value) {
        return (value instanceof Long) + " " + (value instanceof Character c ? "char " + c : "no char")
                + " " + (value instanceof Boolean b && b);
    }

    public static void main(String[] args) {
        System.out.println(describe(41));
        System.out.println(describe("four"));
        System.out.println(describe(2.5));
        System.out.println(describe(Size.LARGE));
        System.out.println(describe(7L));
        System.out.println(describe((Integer) null));
        Comparable<?>[] values = { 3L, 'x', true, false, "s", Size.SMALL, null };
        for (Comparable<?> value : values) System.out.println(kind(value));
    }
}
