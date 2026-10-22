// Constructors of exception classes that throw: the exception they throw goes on from the `new`,
// in place of the one being built, as in `throw new Fatal(diagnostic)` whose diagnostic's text
// fails.

public class ThrowingExceptionConstructors {
    static class Bad extends RuntimeException {
        final int code;

        Bad(int code) {
            super("bad " + code);
            if (code < 0) {
                throw new IllegalStateException("negative code " + code);
            }
            this.code = code;
        }
    }

    static class Worse extends Bad {
        Worse(int code) {
            super(code * 2);
        }
    }

    static class Diagnostic {
        final String key;

        Diagnostic(String key) {
            this.key = key;
        }

        @Override
        public String toString() {
            if (key.isEmpty()) {
                throw new IllegalArgumentException("no key");
            }
            return "diagnostic " + key;
        }
    }

    static class Fatal extends Error {
        Fatal(Diagnostic d) {
            super(d.toString());
        }

        Fatal(Diagnostic d, String where) {
            super(where + ": " + d);
        }
    }

    static class Checked extends Exception {
        Checked(String reason) throws java.io.IOException {
            super(reason);
            if (reason.startsWith("io")) {
                throw new java.io.IOException("cannot even say " + reason);
            }
        }
    }

    static void fail(int code) {
        throw new Bad(code);
    }

    static void check(String reason) throws Checked, java.io.IOException {
        throw new Checked(reason);
    }

    public static void main(String[] args) {
        for (int code : new int[] {3, -1}) {
            try {
                fail(code);
            } catch (Bad e) {
                System.out.println("caught " + e.getMessage() + " with code " + e.code);
            } catch (IllegalStateException e) {
                System.out.println("constructor failed: " + e.getMessage());
            }
        }
        try {
            throw new Worse(-4);
        } catch (IllegalStateException e) {
            System.out.println("subclass constructor failed: " + e.getMessage());
        }
        Worse w = new Worse(5);
        System.out.println("built " + w.getMessage() + " with code " + w.code);
        for (String key : new String[] {"missing", ""}) {
            try {
                throw new Fatal(new Diagnostic(key));
            } catch (Fatal e) {
                System.out.println("fatal: " + e.getMessage());
            } catch (IllegalArgumentException e) {
                System.out.println("no fatal error, but: " + e.getMessage());
            }
            try {
                throw new Fatal(new Diagnostic(key), "reader");
            } catch (Fatal e) {
                System.out.println("fatal: " + e.getMessage());
            } catch (IllegalArgumentException e) {
                System.out.println("no fatal error in reader, but: " + e.getMessage());
            }
        }
        for (String reason : new String[] {"disk full", "io error"}) {
            try {
                check(reason);
            } catch (Checked e) {
                System.out.println("checked: " + e.getMessage());
            } catch (java.io.IOException e) {
                System.out.println("io: " + e.getMessage());
            }
        }
    }
}
