// Objects of `java.lang.Object` itself, as the JDK's collections use them for placeholder values
// and locks: equal only to themselves, hashed by identity, printed as `java.lang.Object@...`,
// stored in `Object` fields, arrays and parameters among strings and boxes, and locked.
public class PlainObjects {
    static final Object PRESENT = new Object();

    private final Object lock = new Object();
    private int count;

    void increment() {
        synchronized (lock) {
            count++;
        }
    }

    static String describe(Object o) {
        if (PRESENT.equals(o)) {
            return "the placeholder";
        }
        if (o instanceof String s) {
            return "the string " + s;
        }
        return "something else";
    }

    public static void main(String[] args) {
        Object a = new Object();
        Object b = new Object();
        System.out.println(a.equals(a) + " " + a.equals(b) + " " + b.equals(a) + " " + a.equals("a"));
        System.out.println((a.hashCode() == a.hashCode()) + " " + (a.hashCode() == b.hashCode()));
        String text = a.toString();
        System.out.println(text.startsWith("java.lang.Object@") + " " + text.equals(String.valueOf(a)));
        System.out.println(text.equals("java.lang.Object@" + Integer.toHexString(a.hashCode())));

        Object[] values = {PRESENT, "word", 3, new Object()};
        for (Object v : values) {
            System.out.println(describe(v));
        }

        PlainObjects counter = new PlainObjects();
        counter.increment();
        counter.increment();
        System.out.println(counter.count);
    }
}
