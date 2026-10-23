package sun.nio.fs;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.Charset;
import java.nio.file.FileSystem;
import java.nio.file.Files;
import java.nio.file.OpenOption;
import java.nio.file.Path;
import java.util.Objects;
import jdk.internal.access.SharedSecrets;

/**
 * The translator's methods that calls of some of the JDK's methods of {@code java.nio.file} call
 * instead ({@code program::REDIRECTED_CALLS}), each with the same type as the JDK's. A
 * {@code Charset} is the runtime's value, which is never null, so these do not test one.
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

    /**
     * {@code Files.readAllBytes(path)}: the bytes of the file, read through
     * {@code Files.newInputStream}. The JDK's reads a channel through a stream of
     * {@code ByteBuffer}s, in an array of the file's size where it can.
     */
    public static byte[] readAllBytes(Path path) throws IOException {
        try (InputStream in = Files.newInputStream(path)) {
            return in.readAllBytes();
        }
    }

    /**
     * {@code Files.readString(path, cs)}: the JDK's, without copying the bytes for a {@code Path}
     * of another module, which it finds through {@code Class.getModule} (there is only the
     * platform's {@code Path} here).
     */
    public static String readString(Path path, Charset cs) throws IOException {
        Objects.requireNonNull(path);
        byte[] ba = Files.readAllBytes(path);
        return SharedSecrets.getJavaLangAccess().uncheckedNewStringOrThrow(ba, cs);
    }

    /** {@code Files.writeString(path, csq, cs, options)}, as {@link #readString}. */
    public static Path writeString(Path path, CharSequence csq, Charset cs, OpenOption... options)
        throws IOException
    {
        Objects.requireNonNull(path);
        Objects.requireNonNull(csq);
        byte[] bytes = SharedSecrets.getJavaLangAccess().uncheckedGetBytesOrThrow(String.valueOf(csq), cs);
        Files.write(path, bytes, options);
        return path;
    }

    /**
     * {@code Files.newBufferedReader(path, cs)}: the JDK's reads with a decoder of {@code cs},
     * which the translation does not have; the translator's {@code InputStreamReader} decodes
     * with {@code cs} itself, and so replaces malformed input where the JDK's reader throws
     * {@code MalformedInputException}.
     */
    public static BufferedReader newBufferedReader(Path path, Charset cs) throws IOException {
        return new BufferedReader(new InputStreamReader(Files.newInputStream(path), cs));
    }

    /**
     * {@code Files.newBufferedWriter(path, cs, options)}: as {@link #newBufferedReader}, a
     * character that {@code cs} cannot encode is replaced where the JDK's writer throws
     * {@code UnmappableCharacterException}.
     */
    public static BufferedWriter newBufferedWriter(Path path, Charset cs, OpenOption... options)
        throws IOException
    {
        return new BufferedWriter(new OutputStreamWriter(Files.newOutputStream(path, options), cs));
    }
}
