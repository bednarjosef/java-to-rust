// `new UncheckedIOException(message, cause)` with a null cause: the constructor rejects it with a
// NullPointerException, which is not caught, so the program exits with code 1 after its output.
import java.io.IOException;
import java.io.UncheckedIOException;

public class UncheckedIoNullMessageCause {
    static UncheckedIOException wrap(String message, IOException cause) {
        return new UncheckedIOException(message, cause);
    }

    public static void main(String[] args) {
        System.out.println("before " + wrap("first", new IOException("a")).getMessage());
        System.out.println("wrapping " + wrap("second", null));
    }
}
