// An instanceof pattern whose binding a later operand of the && passes to a method that throws,
// which a catch sees: the failure leaves the test, as EnumMap's EntrySet.contains does.
public class PatternThenThrowingCall {
    static class Box {
        final int v;

        Box(int v) {
            this.v = v;
        }

        int value() {
            if (v < 0) {
                throw new IllegalStateException("negative " + v);
            }
            return v;
        }
    }

    static boolean positive(Object o) {
        return o instanceof Box b && b.value() > 0;
    }

    static String check(Object o) {
        try {
            return positive(o) ? "positive" : "not positive";
        } catch (IllegalStateException e) {
            return "failed: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(check(new Box(2)));
        System.out.println(check(new Box(0)));
        System.out.println(check("text"));
        System.out.println(check(new Box(-3)));
    }
}
