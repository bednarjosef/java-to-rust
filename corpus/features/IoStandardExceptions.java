// The standard exceptions of `java.io` that the translator provides: `UncheckedIOException` with
// its two constructors and its `IOException` cause, `EOFException` and `UnsupportedEncodingException`,
// thrown and caught as themselves and as `IOException`, with their messages, causes and `toString`.
import java.io.EOFException;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.io.UnsupportedEncodingException;

public class IoStandardExceptions {
    static String readLine(int available) throws IOException {
        if (available == 0) {
            throw new EOFException("end of input");
        }
        return "line";
    }

    static void encode(String charset) throws UnsupportedEncodingException {
        if (!charset.equals("UTF-8")) {
            throw new UnsupportedEncodingException(charset);
        }
    }

    static String readOrWrap(int available) {
        try {
            return readLine(available);
        } catch (IOException e) {
            throw new UncheckedIOException("reading failed", e);
        }
    }

    static void wrapped(IOException e) {
        throw new UncheckedIOException(e);
    }

    public static void main(String[] args) {
        try {
            System.out.println(readOrWrap(1));
            System.out.println(readOrWrap(0));
        } catch (UncheckedIOException e) {
            System.out.println(e.getMessage());
            IOException cause = e.getCause();
            System.out.println(cause.getClass().getSimpleName() + ": " + cause.getMessage());
            System.out.println(e.getCause() == cause);
            System.out.println(e);
        }

        IOException failure = new EOFException();
        UncheckedIOException fromCause = new UncheckedIOException(failure);
        System.out.println(fromCause.getMessage());
        System.out.println(fromCause.getCause() == failure);
        System.out.println(fromCause);

        UncheckedIOException withMessage = new UncheckedIOException("disk", new IOException("full"));
        System.out.println(withMessage.getMessage() + " <- " + withMessage.getCause().getMessage());
        System.out.println(withMessage);

        try {
            wrapped(new EOFException("short"));
        } catch (RuntimeException e) {
            System.out.println("as RuntimeException: " + e);
            System.out.println(e instanceof UncheckedIOException);
        }

        try {
            encode("UTF-8");
            encode("ASCII");
        } catch (IOException e) {
            System.out.println("as IOException: " + e.getMessage());
            System.out.println(e);
            System.out.println(e.getCause());
        }

        try {
            throw new UnsupportedEncodingException();
        } catch (UnsupportedEncodingException e) {
            System.out.println(e.getMessage() + " / " + e);
        }

        EOFException eof = new EOFException("bare");
        IOException asIo = eof;
        System.out.println(asIo.getMessage() + " " + asIo.getCause());
    }
}
