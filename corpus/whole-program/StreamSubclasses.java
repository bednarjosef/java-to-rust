import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.FilterOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintStream;
import java.io.Writer;

// The program's own streams: an OutputStream that counts what it gets, a FilterOutputStream that
// changes it, a ByteArrayOutputStream that reverses its bytes, and an InputStream that makes
// its bytes up, under the JDK's streams and readers.
public class StreamSubclasses {
    static class Counting extends OutputStream {
        int bytes;
        int newlines;

        @Override
        public void write(int b) {
            bytes++;
            if (b == '\n') {
                newlines++;
            }
        }
    }

    static class Upper extends FilterOutputStream {
        Upper(OutputStream out) {
            super(out);
        }

        @Override
        public void write(int b) throws IOException {
            out.write(b >= 'a' && b <= 'z' ? b - 32 : b);
        }
    }

    static class Reversible extends ByteArrayOutputStream {
        void reverse() {
            for (int i = 0, j = count - 1; i < j; i++, j--) {
                byte t = buf[i];
                buf[i] = buf[j];
                buf[j] = t;
            }
        }
    }

    /** A print stream that marks the text printed as a string, which its println goes through. */
    static class Marked extends PrintStream {
        int writes;

        Marked(OutputStream out) {
            super(out, true);
        }

        @Override
        public void print(String s) {
            super.print("> " + s);
        }

        @Override
        public void write(byte[] buf, int off, int len) {
            writes++;
            super.write(buf, off, len);
        }
    }

    /** The lines "row 0" to "row n-1", made up a byte at a time. */
    static class Rows extends InputStream {
        private final int rows;
        private int row;
        private byte[] current = new byte[0];
        private int at;

        Rows(int rows) {
            this.rows = rows;
        }

        @Override
        public int read() {
            if (at == current.length) {
                if (row == rows) {
                    return -1;
                }
                current = ("row " + row++ + "\n").getBytes();
                at = 0;
            }
            return current[at++];
        }
    }

    public static void main(String[] args) throws IOException {
        Counting counting = new Counting();
        PrintStream counted = new PrintStream(counting);
        counted.println("one line");
        counted.printf("%s and %d%n", "another", 2);
        counted.print("no newline");
        counted.flush();
        System.out.println(counting.bytes + " bytes, " + counting.newlines + " newlines");

        PrintStream shouting = new PrintStream(new Upper(System.out), true);
        shouting.println("quiet words");
        shouting.write('x');
        shouting.println();

        Marked marked = new Marked(System.out);
        marked.println("a string");
        marked.println(42);
        marked.println((Object) "an object");
        marked.print("then");
        marked.println();
        marked.println(new char[] {'c', 'h'});
        System.out.println("writes: " + marked.writes);

        Reversible reversible = new Reversible();
        reversible.write("stressed".getBytes());
        reversible.reverse();
        System.out.println(reversible.toString() + " " + reversible.size());
        reversible.writeTo(System.out);
        System.out.println();

        // A writer that is closed flushes and closes its stream, and has no encoding after.
        ByteArrayOutputStream encoded = new ByteArrayOutputStream();
        OutputStreamWriter closing = new OutputStreamWriter(encoded, "ISO-8859-1");
        closing.write("caf\u00e9 \u20ac");
        System.out.println("before close: " + encoded.size() + " bytes, " + closing.getEncoding());
        closing.close();
        System.out.println("after close: " + encoded.size() + " bytes, " + closing.getEncoding());

        BufferedReader rows = new BufferedReader(new InputStreamReader(new Rows(1000)));
        String line;
        String last = null;
        int lines = 0;
        while ((line = rows.readLine()) != null) {
            lines++;
            last = line;
        }
        System.out.println(lines + " lines, the last " + last);
        Rows few = new Rows(2);
        byte[] some = new byte[100];
        int n = few.read(some, 0, some.length);
        System.out.println("read " + n + ": " + new String(some, 0, n).trim().replace('\n', '|'));
        System.out.println("skipped " + new Rows(3).skip(10));

        // The JDK's message of a call on a null in its own code.
        try {
            new FilterOutputStream(null).write(1);
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }

        // Closing a writer over System.out closes System.out: nothing printed after reaches it.
        Writer latin = new OutputStreamWriter(System.out, "ISO-8859-1");
        latin.write("latin and \u00e9 and \u20ac\n");
        latin.close();
        System.out.println("System.out after its writer closed it");
        System.err.println("standard error still open");
    }
}
