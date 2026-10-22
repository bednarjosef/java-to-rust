// Enums whose constants are built by a constructor that may throw, used inside a `try` that
// catches `Throwable`: the first use sees `ExceptionInInitializerError` with its cause, and a
// later one `NoClassDefFoundError`, as Java's class initialization gives them, whether or not the
// enum has static initialization of its own. An enum whose constants build without failing is
// used inside the same kind of `try`.
public class FailingEnumConstants {
    enum Ratio {
        HALF(divisor(2)), WHOLE(divisor(1)), BROKEN(divisor(0));

        static final String NAMES = names();

        final int value;

        Ratio(int d) {
            value = 100 / d;
        }

        static int divisor(int d) {
            return d;
        }

        static String names() {
            return HALF + "," + WHOLE;
        }
    }

    enum Level {
        LOW(depth(1)), HIGH(depth(-1));

        final int steps;

        Level(int n) {
            steps = depth(n);
        }

        static int depth(int n) {
            if (n < 0) {
                throw new IllegalStateException("negative depth " + n);
            }
            return n;
        }
    }

    enum Scale {
        SMALL(size(1)), LARGE(size(10));

        static final int TOTAL = SMALL.factor + LARGE.factor;

        final int factor;

        Scale(int n) {
            factor = 1000 / n;
        }

        static int size(int n) {
            return n;
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println(Scale.LARGE.factor + " " + Scale.SMALL.factor + " " + Scale.TOTAL);
        } catch (Throwable t) {
            System.out.println("scale: " + t);
        }
        for (int i = 0; i < 2; i++) {
            try {
                System.out.println(Ratio.HALF.value + " " + Ratio.NAMES);
            } catch (Throwable t) {
                System.out.println("ratio: " + t + " cause " + t.getCause());
            }
            try {
                System.out.println(Level.LOW.steps);
            } catch (Throwable t) {
                System.out.println("level: " + t + " cause " + t.getCause());
            }
        }
    }
}
