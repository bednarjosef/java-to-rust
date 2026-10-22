package java.nio.file;

/**
 * The translator's patch of the JDK's {@code InvalidPathException} (see docs/jdk-overlays.md). A
 * translation cannot build an exception whose constructor throws (see
 * {@code analysis/exceptions.rs}), and the JDK's throws {@code NullPointerException} for a null
 * input or reason and {@code IllegalArgumentException} for an index below -1. Patched, it takes
 * its arguments as they are, as the JDK's own callers pass them; a program that passes such
 * arguments itself gets the exception where Java would throw another. Its fields are final, as
 * nothing assigns them after the constructor.
 */
@Edit(find = "private String input;", replace = "private final String input;")
@Edit(find = "private int index;", replace = "private final int index;")
class InvalidPathException {
    @Edit(from = "if ((input == null) || (reason == null))", to = "throw new IllegalArgumentException();", replace = "")
    public InvalidPathException(String input, String reason, int index);
}
