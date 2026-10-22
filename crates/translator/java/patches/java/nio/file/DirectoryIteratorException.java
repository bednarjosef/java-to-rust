package java.nio.file;

/**
 * The translator's patch of the JDK's {@code DirectoryIteratorException} (see
 * docs/jdk-overlays.md). A translation cannot build an exception whose constructor throws (see
 * {@code analysis/exceptions.rs}), and the JDK's throws {@code NullPointerException} for a null
 * cause. Patched, it takes the cause as it is, as the JDK's own callers pass it; a program that
 * passes null itself gets the exception where Java would throw a {@code NullPointerException}.
 * There is no {@code readObject}, as a translated program does not deserialize.
 */
class DirectoryIteratorException {
    @Edit(find = "super(Objects.requireNonNull(cause));", replace = "super(cause);")
    public DirectoryIteratorException(IOException cause);

    @Remove
    private void readObject(ObjectInputStream s);
}
