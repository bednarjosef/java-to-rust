// `throw` of a value that may be null throws a `NullPointerException` instead: a `catch` sees it,
// a `finally` runs for it, and nothing catches it in `main`, which ends the program.
public class ThrowMaybeNull {
    static class Problem extends RuntimeException {
        Problem(String message) {
            super(message);
        }
    }

    static RuntimeException make(int n) {
        return n > 0 ? new Problem("problem " + n) : null;
    }

    static void fail(RuntimeException e) {
        throw e;
    }

    static void caught(int n) {
        try {
            fail(make(n));
        } catch (NullPointerException e) {
            System.out.println("null " + n);
        } catch (Problem e) {
            System.out.println("problem " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        caught(1);
        caught(0);
        RuntimeException none = null;
        try {
            throw none;
        } catch (RuntimeException e) {
            System.out.println(e instanceof NullPointerException);
        } finally {
            System.out.println("finally");
        }
        try {
            throw null;
        } catch (NullPointerException e) {
            System.out.println("throw null");
        }
        System.out.println("before");
        throw make(0);
    }
}
