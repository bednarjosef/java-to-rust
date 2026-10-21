// `AssertionError`s thrown with each kind of detail and caught as `AssertionError` and as
// `Throwable`: the message and cause of each, a chain walked with `getCause`, and one whose
// cause is a program's own exception.
public class AssertionErrorThrown {
    static class Limit extends Exception {
        Limit(String message) {
            super(message);
        }
    }

    static void check(int value) {
        if (value < 0) {
            throw new AssertionError(value);
        }
        if (value > 100) {
            throw new AssertionError("too big: " + value, new Limit("limit 100"));
        }
    }

    static void verify(Object detail) {
        throw new AssertionError(detail);
    }

    static String describe(Throwable t) {
        String chain = t.getClass().getSimpleName() + "(" + t.getMessage() + ")";
        for (Throwable c = t.getCause(); c != null; c = c.getCause()) {
            chain += " <- " + c.getClass().getSimpleName() + "(" + c.getMessage() + ")";
        }
        return chain;
    }

    public static void main(String[] args) {
        int[] values = {5, -3, 250};
        for (int v : values) {
            try {
                check(v);
                System.out.println("ok " + v);
            } catch (AssertionError e) {
                System.out.println("caught " + e);
                System.out.println(describe(e));
            } catch (Throwable t) {
                System.out.println("not reached: " + t);
            }
        }

        try {
            verify(new Limit("from object"));
        } catch (AssertionError e) {
            System.out.println(e.getMessage());
            System.out.println(e.getCause() instanceof Limit);
            System.out.println(describe(e));
        }

        try {
            verify(2.5);
        } catch (Throwable t) {
            System.out.println(t + " / " + t.getCause());
        }

        try {
            throw new AssertionError('x');
        } catch (AssertionError e) {
            System.out.println(e.getMessage().length() + " " + e.getMessage());
        }

        try {
            throw new AssertionError(new IllegalArgumentException("bad input"));
        } catch (Error e) {
            System.out.println(e.getMessage());
            System.out.println(e.getCause().getMessage());
        } finally {
            System.out.println("finally");
        }
    }
}
