// Which catch clause runs: subclasses before their superclass, an exception that overrides
// getMessage or toString, exceptions thrown through virtual calls, a rethrow wrapping the cause,
// getCause and getClass().getName(), jumps out of a try to labeled loops, nested trys, and a
// caught exception kept in a variable.
public class CatchOrder {
    static class AppException extends Exception {
        AppException(String message) {
            super(message);
        }

        AppException(String message, Throwable cause) {
            super(message, cause);
        }
    }

    static class NotFound extends AppException {
        final String key;

        NotFound(String key) {
            super("no such key: " + key);
            this.key = key;
        }
    }

    static class Invalid extends AppException {
        Invalid(String message) {
            super(message);
        }

        @Override
        public String getMessage() {
            return "invalid(" + super.getMessage() + ")";
        }
    }

    static class Fatal extends RuntimeException {
        Fatal(String message) {
            super(message);
        }

        @Override
        public String toString() {
            return "Fatal!" + getMessage();
        }
    }

    abstract static class Step {
        abstract int run(int x) throws AppException;
    }

    static class Lookup extends Step {
        int run(int x) throws AppException {
            if (x > 10) {
                throw new NotFound("k" + x);
            }
            return x * 2;
        }
    }

    static class Check extends Step {
        int run(int x) throws AppException {
            if (x % 2 == 1) {
                throw new Invalid("odd " + x);
            }
            if (x == 4) {
                throw new Fatal("four");
            }
            return x + 1;
        }
    }

    static int pipeline(Step[] steps, int x) throws AppException {
        int value = x;
        for (Step s : steps) {
            value = s.run(value);
        }
        return value;
    }

    static String classify(Step[] steps, int x) {
        String result;
        try {
            result = "value " + pipeline(steps, x);
        } catch (NotFound e) {
            result = "not found " + e.key;
        } catch (AppException e) {
            result = "app " + e.getMessage();
        } catch (RuntimeException e) {
            result = "runtime " + e;
        }
        return result;
    }

    static void rethrow(int x) throws AppException {
        try {
            if (x < 0) {
                throw new Invalid("negative");
            }
            System.out.println("non-negative " + x);
        } catch (Invalid e) {
            System.out.println("rethrowing " + e.getMessage());
            throw new AppException("wrapped", e);
        }
    }

    public static void main(String[] args) {
        Step[] steps = { new Check(), new Lookup() };
        for (int x : new int[] { 2, 3, 4, 6, 8 }) {
            System.out.println(x + ": " + classify(steps, x));
        }
        for (int x = -1; x <= 1; x++) {
            try {
                rethrow(x);
            } catch (AppException e) {
                Throwable cause = e.getCause();
                System.out.println(e.getMessage() + " caused by " + cause);
                System.out.println(cause.getMessage());
                System.out.println(cause.getClass().getName());
            }
        }
        int found = 0;
        outer:
        for (int i = 0; i < 5; i++) {
            for (int j = 0; j < 5; j++) {
                try {
                    if (i * j == 6) {
                        found = i * 10 + j;
                        break outer;
                    }
                    if (j > i) {
                        continue outer;
                    }
                    if (i == 4) {
                        throw new Fatal("i is 4");
                    }
                } catch (Fatal e) {
                    System.out.println("inner " + e);
                    break;
                }
            }
        }
        System.out.println("found " + found);
        try {
            try {
                new Lookup().run(20);
            } catch (Invalid e) {
                System.out.println("not here");
            }
        } catch (NotFound e) {
            System.out.println("outer caught " + e.getMessage());
        } catch (AppException e) {
            System.out.println("not here either");
        }
        Exception saved = null;
        try {
            new Check().run(5);
        } catch (Exception e) {
            saved = e;
        }
        System.out.println("saved " + saved);
        Throwable t = new Error("an error");
        System.out.println(t + " " + t.getClass().getName());
        RuntimeException r = new IllegalStateException("state");
        try {
            throw r;
        } catch (IllegalStateException e) {
            System.out.println("same object " + (e == r));
        }
    }
}
