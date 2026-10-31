// Throws of an exception that a call makes: `new` of an exception class with subclasses (which
// the translation builds with a factory), a static helper that makes the exception, and a method
// of an object that does. Each is made out of line, as a `new` is; the message, the order of the
// arguments' effects and the catch are Java's.
public class ThrownFromCalls {
    static class Problem extends RuntimeException {
        final int code;

        Problem(String message, int code) {
            super(message + " (" + code + ")");
            this.code = code;
        }
    }

    static class BadInput extends Problem {
        BadInput(String message) {
            super("bad input: " + message, 400);
        }
    }

    static class Reporter {
        int made;

        IllegalStateException make(String what, int at) {
            made++;
            return new IllegalStateException(what + " at " + at + ", report " + made);
        }
    }

    static int calls;

    static String note(String text) {
        calls++;
        return text + "#" + calls;
    }

    static IllegalArgumentException outOfRange(long value, long start, long end) {
        return new IllegalArgumentException(
            String.format("The value %s is not in the specified inclusive range of %s to %s", value, start, end));
    }

    static void inclusiveBetween(long start, long end, long value) {
        if (value < start || value > end) {
            throw outOfRange(value, start, end);
        }
    }

    static void check(int code) {
        if (code > 2) {
            throw new Problem(note("code too high"), code);
        }
        if (code < 0) {
            throw new BadInput(note("negative"));
        }
    }

    public static void main(String[] args) {
        Reporter reporter = new Reporter();
        long sum = 0;
        for (int i = -2; i < 6; i++) {
            try {
                inclusiveBetween(0, 3, i);
                check(i);
                if (i == 2) {
                    throw reporter.make(note("two"), i);
                }
                sum += i;
            } catch (BadInput e) {
                System.out.println("bad " + e.code + ": " + e.getMessage());
            } catch (Problem e) {
                System.out.println("problem " + e.code + ": " + e.getMessage());
            } catch (RuntimeException e) {
                System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
            }
        }
        try {
            check(-1);
        } catch (Problem e) {
            System.out.println(e.getClass().getSimpleName() + " " + e.code + ": " + e.getMessage());
        }
        try {
            check(7);
        } catch (RuntimeException e) {
            System.out.println(e.getMessage());
        }
        System.out.println("sum " + sum + ", calls " + calls + ", reports " + reporter.made);
    }
}
