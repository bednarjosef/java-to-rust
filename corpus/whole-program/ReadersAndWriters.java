import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.io.Reader;
import java.io.Writer;
import java.nio.charset.StandardCharsets;

// Readers over System.in and over bytes in memory, and writers over System.out: lines, text
// that is not ASCII split across reads, charsets, and what a writer that is never flushed loses.
public class ReadersAndWriters {
    public static void main(String[] args) throws IOException {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
        String line;
        int count = 0;
        while ((line = in.readLine()) != null) {
            count++;
            System.out.println(count + " " + line.length() + " " + line);
        }
        System.out.println("ready at the end: " + in.ready());

        // More than one buffer of UTF-8, with characters of two, three and four bytes, read in
        // pieces of every size.
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < 4000; i++) {
            text.append(i % 7 == 0 ? "é" : i % 11 == 0 ? "€" : i % 13 == 0 ? "😀" : "a");
            if (i % 50 == 49) {
                text.append('\n');
            }
        }
        byte[] utf8 = text.toString().getBytes(StandardCharsets.UTF_8);
        Reader reader = new InputStreamReader(new ByteArrayInputStream(utf8), StandardCharsets.UTF_8);
        char[] chunk = new char[7];
        StringBuilder back = new StringBuilder();
        int n;
        while ((n = reader.read(chunk, 0, chunk.length)) != -1) {
            back.append(chunk, 0, n);
        }
        System.out.println("decoded " + back.length() + " chars, same: " + back.toString().equals(text.toString()));
        byte[] latin = {'c', 'a', 'f', (byte) 0xe9};
        BufferedReader latinReader =
                new BufferedReader(new InputStreamReader(new ByteArrayInputStream(latin), "ISO-8859-1"));
        System.out.println("latin: " + latinReader.readLine());
        System.out.println("encoding: " + new InputStreamReader(System.in).getEncoding());

        BufferedWriter out = new BufferedWriter(new OutputStreamWriter(System.out));
        for (int i = 0; i < 3; i++) {
            out.write("line " + i);
            out.newLine();
        }
        System.out.println("direct, before the writer's flush");
        out.flush();
        out.write("café € 😀");
        out.write('\n');
        out.flush();

        PrintWriter pw = new PrintWriter(System.out);
        pw.println("print writer " + 42);
        pw.print('x');
        pw.println(2.5);
        pw.flush();

        PrintWriter auto = new PrintWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8), true);
        auto.println("auto flushed");
        System.out.println("after auto");

        Writer ascii = new OutputStreamWriter(System.out, StandardCharsets.US_ASCII);
        ascii.write("ascii and é and 😀\n");
        ascii.flush();
        OutputStreamWriter latinWriter = new OutputStreamWriter(System.out, "ISO-8859-1");
        latinWriter.write("latin and é and €\n");
        latinWriter.flush();
        System.out.println("encoding: " + latinWriter.getEncoding());

        // The writer's buffer holds what it was given until it is flushed, so these lines are
        // lost when the program ends, in Java as in the translation.
        Writer w = new OutputStreamWriter(System.out);
        w.write("this is lost: the writer is never flushed\n");
        System.out.println("end");
        // Closing the writer closes System.out under it, so nothing after it is printed.
        latinWriter.close();
        System.out.println("not shown: System.out is closed " + latinWriter.getEncoding());
    }
}
