/**
 * A `catch` clause of an exception that nothing creates never runs, and neither does what only it
 * calls: here a method that asks for its class's module, which is not translated. The clauses of
 * exceptions that the code throws, of those that the runtime raises, and of the linkage error of
 * a static initializer that fails do run; one of a linkage error that nothing raises does not.
 */
public class DeadCatches {
    static class Rare extends RuntimeException {
        Rare(String message) {
            super(message);
        }
    }

    static class Common extends RuntimeException {
        Common(String message) {
            super(message);
        }
    }

    /** A class whose static initialization fails. */
    static class Faulty {
        static final int VALUE = compute();

        static int compute() {
            if ("broken".length() > 0) {
                throw new IllegalStateException("broken");
            }
            return 1;
        }
    }

    static String module() {
        return String.valueOf(DeadCatches.class.getModule());
    }

    static int check(int n) {
        if (n < 0) {
            throw new Common("negative " + n);
        }
        return 10 / n;
    }

    public static void main(String[] args) {
        for (int n : new int[] {5, -1, 0}) {
            try {
                System.out.println("quotient " + check(n));
            } catch (Rare e) {
                System.out.println("rare in " + module());
            } catch (Common e) {
                System.out.println("common: " + e.getMessage());
            } catch (ArithmeticException e) {
                System.out.println("arithmetic: " + e.getMessage());
            }
        }
        try {
            System.out.println("value " + Faulty.VALUE);
        } catch (IllegalAccessError e) {
            System.out.println("illegal access in " + module());
        } catch (ExceptionInInitializerError e) {
            System.out.println("initializer: " + e.getCause().getMessage());
        }
    }
}
