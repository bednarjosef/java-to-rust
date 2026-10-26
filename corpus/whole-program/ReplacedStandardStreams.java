import java.io.ByteArrayOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;

// System.setOut and System.setErr, as a Minecraft server's Bootstrap wraps both streams in a
// PrintStream that logs each line: printing calls written on System.out and System.err go to the
// stream set, also those the program makes before it sets one.
public class ReplacedStandardStreams {
    static class Logged extends PrintStream {
        private final String name;

        Logged(String name, OutputStream out) {
            super(out);
            this.name = name;
        }

        @Override
        public void println(String s) {
            super.println("[" + name + "] " + s);
        }

        @Override
        public void println(Object o) {
            println(String.valueOf(o));
        }
    }

    static void report(String when) {
        System.out.println("out " + when);
        System.out.println(42);
        System.out.print('c');
        System.out.println(new char[] {'h', 'i'});
        System.out.printf("%d-%s%n", 7, when);
        System.out.flush();
        System.err.println("err " + when);
        System.err.flush();
    }

    public static void main(String[] args) {
        PrintStream out = System.out;
        PrintStream err = System.err;
        report("before");
        System.setOut(new Logged("STDOUT", out));
        System.setErr(new Logged("STDERR", out));
        report("after");
        System.out.println((Object) "an object");
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        System.setOut(new PrintStream(bytes, true));
        System.out.println("captured");
        System.setOut(out);
        System.setErr(err);
        System.out.println("kept " + bytes.toString().trim());
        report("restored");
    }
}
