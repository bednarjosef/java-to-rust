import java.io.BufferedInputStream;
import java.io.IOException;
import java.io.InputStream;

// System.in as an InputStream: single bytes, arrays, ranges, available and skip, and wrapped in
// a BufferedInputStream of its own, reading exactly what Java reads from the input file.
public class StandardInputBytes {
    static InputStream source = System.in;

    public static void main(String[] args) throws IOException {
        int first = System.in.read();
        System.out.println("first " + first + " " + (char) first);
        byte[] four = new byte[4];
        int n = source.read(four);
        System.out.println("read " + n + " " + new String(four, 0, n));
        byte[] buf = new byte[10];
        n = System.in.read(buf, 3, 5);
        System.out.println("range " + n + " " + new String(buf, 3, n));
        System.out.println("nothing asked: " + System.in.read(buf, 0, 0));
        System.out.println("available " + (System.in.available() > 0));
        System.out.println("skipped " + System.in.skip(2));
        System.out.println("marks: " + System.in.markSupported());
        System.in.mark(100);
        int before = System.in.read();
        System.in.reset();
        System.out.println("read again after reset: " + (System.in.read() == before));

        InputStream in = new BufferedInputStream(System.in, 16);
        int lines = 0;
        int bytes = 0;
        int sum = 0;
        int b;
        while ((b = in.read()) != -1) {
            bytes++;
            sum += b;
            if (b == '\n') {
                lines++;
            }
        }
        System.out.println(lines + " lines, " + bytes + " bytes, sum " + sum);
        System.out.println("at the end: " + System.in.read() + " " + System.in.available());
        try {
            System.in.read(buf, 8, 5);
        } catch (IndexOutOfBoundsException e) {
            System.out.println("out of bounds");
        }
    }
}
