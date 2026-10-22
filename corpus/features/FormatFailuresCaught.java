import java.util.IllegalFormatException;
import java.util.MissingFormatArgumentException;
import java.util.UnknownFormatConversionException;

// A format that is not a literal may not fit its arguments; a `catch` sees the exception of
// `java.util` that Java throws, with Java's message, and the exceptions a program makes itself
// describe what they were given.
public class FormatFailuresCaught {
    static String format(String format, String arg) {
        try {
            return String.format(format, arg);
        } catch (MissingFormatArgumentException e) {
            return "missing: " + e.getMessage();
        } catch (IllegalFormatException e) {
            return e.getClass().getName() + ": " + e.getMessage();
        }
    }

    // The arguments in an `Object[]`, as `LocaleResources.trace` passes them.
    static String trace(String format, Object... args) {
        try {
            return String.format(format, args);
        } catch (IllegalArgumentException e) {
            return "trace: " + e.getMessage();
        }
    }

    static void print(String format, int value) {
        try {
            System.out.printf(format, value);
            System.out.println();
        } catch (IllegalArgumentException e) {
            System.out.println("printf: " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        String[] formats = {"%s and %s", "%q", "%d", "%-d", "%,s", "%.2d", "%--s", "%#s", "%c"};
        for (String f : formats) {
            System.out.println(format(f, "x"));
        }
        System.out.println(trace("%s and %s", "x", "y"));
        System.out.println(trace("%s and %s", "x"));
        print("%d%%", 7);
        print("%2$s", 7);
        print("%z", 7);
        print("%c", 0x110000);
        try {
            System.out.println("%d".formatted("text"));
        } catch (RuntimeException e) {
            System.out.println("formatted: " + e.getClass().getSimpleName() + " " + e.getMessage());
        }
        IllegalFormatException made = new UnknownFormatConversionException("w");
        System.out.println(made.getMessage() + " " + ((UnknownFormatConversionException) made).getConversion());
        System.out.println(new MissingFormatArgumentException("%7s").getFormatSpecifier());
    }
}
