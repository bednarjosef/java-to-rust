// A method of an enum called on an `Object` cast to the enum where a test has found it is not
// null, inside a `try` that catches `NullPointerException`: the call cannot throw it.
public class EnumCallOnTestedObject {
    enum Flag { A, B; long value() { return ordinal() + 10; } public String toString() { return "flag" + ordinal(); } }
    static Object pick(int k) { return k > 0 ? Flag.B : null; }
    static Object call(Object r) {
        Object result;
        if (r != null) {
            result = ((Flag) r).toString();
        } else {
            result = null;
        }
        return result;
    }
    static Object value(Object r) {
        return r != null ? ((Flag) r).value() : null;
    }
    public static void main(String[] args) {
        try {
            System.out.println(call(pick(1)));
            System.out.println(call(pick(0)));
            System.out.println(value(pick(1)));
            System.out.println(((Flag) pick(0)).value());
        } catch (NullPointerException e) {
            System.out.println("npe");
        }
    }
}
