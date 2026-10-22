package java.nio.file;

import java.io.IOException;
import java.util.ConcurrentModificationException;

/**
 * The translator's {@code DirectoryIteratorException}, which replaces the JDK's in a translation
 * with the JDK's sources. A translation cannot build an exception whose constructor throws (see
 * {@code analysis/exceptions.rs}), and the JDK's throws {@code NullPointerException} for a null
 * cause. This one takes the cause as it is, as the JDK's own callers pass it; a program that
 * passes null itself gets the exception where Java would throw a {@code NullPointerException}.
 * There is no {@code readObject}, as a translated program does not deserialize. The rest is the
 * JDK's.
 */
public final class DirectoryIteratorException extends ConcurrentModificationException {
    @java.io.Serial
    private static final long serialVersionUID = -6012699886086212874L;

    public DirectoryIteratorException(IOException cause) {
        super(cause);
    }

    @Override
    public IOException getCause() {
        return (IOException) super.getCause();
    }
}
