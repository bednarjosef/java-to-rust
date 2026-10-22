package sun.nio.fs;

import java.nio.file.FileSystem;

/**
 * The translator's methods that calls of some of the JDK's methods of {@code java.nio.file} call
 * instead ({@code program::REDIRECTED_CALLS}), each with the same type as the JDK's.
 */
public final class TranslatedFiles {
    private TranslatedFiles() {}

    /**
     * {@code FileSystems.DefaultFileSystemHolder.defaultFileSystem()}: the platform's file
     * system. The JDK's looks for another provider first, which the property
     * {@code java.nio.file.spi.DefaultFileSystemProvider} would name and reflection create (a
     * translated program does not have that property), and asks the provider for the file system
     * of the URI {@code file:///}, which is this one.
     */
    public static FileSystem defaultFileSystem() {
        return DefaultFileSystemProvider.theFileSystem();
    }
}
