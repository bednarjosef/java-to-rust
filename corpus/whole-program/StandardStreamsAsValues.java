import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.io.PrintStream;

// System.out and System.err as values: in locals, fields, statics and parameters, wrapped in
// other streams, and closed, in order with the calls made on System.out itself.
public class StandardStreamsAsValues {
    static PrintStream shared = System.out;

    private final PrintStream target;

    StandardStreamsAsValues(PrintStream target) {
        this.target = target;
    }

    void report(String label, int n) {
        target.println(label + "=" + n);
        target.print('c');
        target.print(3.5);
        target.print(true);
        target.print(new char[] {'!', '?'});
        target.println();
        target.printf("%d-%s|%5.2f%n", n, label, 2.0 / 3);
        target.format("%x%n", 255).println("chained");
    }

    static void bytesTo(OutputStream raw) throws IOException {
        byte[] bytes = "bytes\n".getBytes();
        raw.write(bytes);
        raw.write(bytes, 1, 3);
        raw.write('\n');
    }

    public static void main(String[] args) throws IOException {
        PrintStream out = System.out;
        out.println("hello");
        System.out.println("direct");
        new StandardStreamsAsValues(out).report("a", 1);
        shared.println(42L);
        shared.append("appended ").append('x').append("abcdef", 1, 3).println();
        bytesTo(System.out);
        System.out.write('x');
        System.out.write('\n');
        System.out.write("written\n".getBytes(), 0, 8);
        System.out.writeBytes("whole array\n".getBytes());

        PrintStream err = System.err;
        err.println("to standard error");
        System.err.printf("formatted %d to standard error%n", 7);

        // A buffered stream of its own: its line comes out when it is flushed, after a line
        // printed later.
        PrintStream buffered = new PrintStream(new BufferedOutputStream(System.out));
        buffered.println("buffered line");
        System.out.println("printed after the buffered line, before its flush");
        buffered.flush();
        System.out.println("errors so far: " + out.checkError());

        try {
            out.write(new byte[2], 1, 5);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of bounds: " + e.getMessage());
        }
        try {
            out.write(new byte[300], 200, 150);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("long range out of bounds: " + e.getMessage());
        }
        try {
            out.write(new byte[3], -1, 2);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("negative offset: " + e.getMessage());
        }

        // A byte that waits in System.out's buffer comes out when the stream is closed; nothing
        // printed after that does, and the stream reports the error.
        out.write('z');
        out.close();
        System.out.println("after close");
        out.println("after close, as a value");
        System.err.println("closed has an error: " + System.out.checkError());
    }
}
