package java.nio.file;

/**
 * The translator's {@code InvalidPathException}, which replaces the JDK's in a translation with
 * the JDK's sources. A translation cannot build an exception whose constructor throws (see
 * {@code analysis/exceptions.rs}), and the JDK's throws {@code NullPointerException} for a null
 * input or reason and {@code IllegalArgumentException} for an index below -1. This one takes its
 * arguments as they are, as the JDK's own callers pass them; a program that passes such
 * arguments itself gets the exception where Java would throw another. The rest is the JDK's.
 */
public class InvalidPathException extends IllegalArgumentException {
    static final long serialVersionUID = 4355821422286746137L;

    private final String input;
    private final int index;

    public InvalidPathException(String input, String reason, int index) {
        super(reason);
        this.input = input;
        this.index = index;
    }

    public InvalidPathException(String input, String reason) {
        this(input, reason, -1);
    }

    public String getInput() {
        return input;
    }

    public String getReason() {
        return super.getMessage();
    }

    public int getIndex() {
        return index;
    }

    /** The reason, the index where there is one, and the input. */
    public String getMessage() {
        StringBuilder sb = new StringBuilder();
        sb.append(getReason());
        if (index > -1) {
            sb.append(" at index ");
            sb.append(index);
        }
        sb.append(": ");
        sb.append(input);
        return sb.toString();
    }
}
