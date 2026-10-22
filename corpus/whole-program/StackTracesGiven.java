import java.io.PrintWriter;
import java.io.StringWriter;

// Stack traces that the program gives its exceptions with `setStackTrace`, printed to standard
// output, to a writer and as text: Java prints them as given, with the frames that a cause or a
// suppressed exception has in common with the one around it elided. (Without `setStackTrace`, a
// translation's traces are empty, which Java's are not.) And a logger-like method that names the
// class that calls it through a stack walker held in a field.
public class StackTracesGiven {
    private static final StackWalker WALKER = StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE);

    static StackTraceElement frame(String method, int line) {
        return new StackTraceElement("app.Server", method, "Server.java", line);
    }

    static <T extends Throwable> T traced(T e, StackTraceElement... frames) {
        e.setStackTrace(frames);
        return e;
    }

    static String loggerName() {
        return "logger of " + WALKER.getCallerClass().getName();
    }

    static class Handler {
        final String logger = loggerName();
    }

    public static void main(String[] args) {
        StackTraceElement main = new StackTraceElement("app.Main", "main", "Main.java", 7);
        StackTraceElement run = frame("run", 40);
        StackTraceElement handle = frame("handle", 61);
        IllegalStateException root = traced(new IllegalStateException("disk full"), frame("write", 88), handle, run, main);
        RuntimeException failure = traced(new RuntimeException("request failed", root), handle, run, main);
        failure.addSuppressed(traced(new IllegalArgumentException("close failed"), frame("close", 99), run, main));
        failure.addSuppressed(traced(new UnsupportedOperationException(), main));
        failure.printStackTrace(System.out);

        StringWriter text = new StringWriter();
        failure.printStackTrace(new PrintWriter(text));
        System.out.println(text.toString().equals(textOf(failure)));
        System.out.print(text);

        StackTraceElement[] got = failure.getStackTrace();
        got[0] = null;
        System.out.println(failure.getStackTrace()[0] + " " + got.length);
        System.out.println(new StackTraceElement("A", "native", null, -2) + " " + new StackTraceElement("A", "b", "A.java", -1));
        try {
            failure.setStackTrace(new StackTraceElement[] {main, null});
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            new StackTraceElement(null, "m", "F.java", 1);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        Exception empty = traced(new Exception("no frames", traced(new Exception("cause"))));
        empty.printStackTrace(System.out);
        System.out.println(empty.getStackTrace().length + " " + (empty.fillInStackTrace() == empty));

        System.out.println(loggerName());
        System.out.println(new Handler().logger);
    }

    static String textOf(Throwable e) {
        StringWriter out = new StringWriter();
        PrintWriter writer = new PrintWriter(out);
        e.printStackTrace(writer);
        writer.flush();
        return out.toString();
    }
}
