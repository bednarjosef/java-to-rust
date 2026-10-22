// Stack traces: printed to standard error (which the corpus does not compare: Java prints frames
// there, a translation prints none), given with `setStackTrace` and read back, and an uncaught
// exception with a cause and suppressed exceptions, which ends the program with exit code 1.
public class StackTracesPrinted {
    static class Quiet extends RuntimeException {
        Quiet(String message) {
            super(message);
        }

        @Override
        public Throwable fillInStackTrace() {
            return this;
        }
    }

    static class Resource implements AutoCloseable {
        private final String name;

        Resource(String name) {
            this.name = name;
        }

        @Override
        public void close() {
            throw new IllegalStateException("closing " + name);
        }
    }

    public static void main(String[] args) {
        RuntimeException failure =
                new RuntimeException("failed", new IllegalArgumentException("bad", new ArithmeticException("zero")));
        failure.addSuppressed(new IllegalStateException("first", new RuntimeException("inner")));
        failure.addSuppressed(new UnsupportedOperationException());
        failure.printStackTrace();

        Exception first = new Exception("first");
        Exception second = new Exception("second", first);
        first.initCause(second);
        second.printStackTrace();

        StackTraceElement[] none = new StackTraceElement[0];
        failure.setStackTrace(none);
        System.out.println(failure.getStackTrace().length);
        StackTraceElement here = new StackTraceElement("app.Main", "main", "Main.java", 12);
        failure.setStackTrace(new StackTraceElement[] {here, new StackTraceElement("app.Main", "run", null, -1)});
        StackTraceElement[] got = failure.getStackTrace();
        System.out.println(got.length + " " + got[0] + " " + got[1]);
        System.out.println(here.getClassName() + " " + here.getMethodName() + " " + here.getFileName() + " "
                + here.getLineNumber() + " " + here.isNativeMethod());
        System.out.println(new StackTraceElement("app.Main", "read", null, -2));
        got[0] = null;
        System.out.println(failure.getStackTrace()[0]);
        failure.printStackTrace();
        try {
            failure.setStackTrace(got);
        } catch (NullPointerException e) {
            System.out.println("null frame: " + e.getMessage());
        }
        try {
            System.out.println(new StackTraceElement("app.Main", null, "Main.java", 1));
        } catch (NullPointerException e) {
            System.out.println("null method: " + e.getMessage());
        }

        Quiet quiet = new Quiet("quiet");
        System.out.println(quiet.fillInStackTrace() == quiet);
        System.out.println(failure.fillInStackTrace() == failure);

        try (Resource a = new Resource("a"); Resource b = new Resource("b")) {
            throw new RuntimeException("body", quiet);
        }
    }
}
