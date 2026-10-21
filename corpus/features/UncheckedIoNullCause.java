// `new UncheckedIOException(cause)` with a null cause: the constructor rejects it with a
// NullPointerException, which is not caught, so the program exits with code 1 after its output.
import java.io.IOException;
import java.io.UncheckedIOException;

public class UncheckedIoNullCause {
    public static void main(String[] args) {
        System.out.println("before");
        IOException none = null;
        UncheckedIOException e = new UncheckedIOException(none);
        System.out.println("unreachable " + e);
    }
}
