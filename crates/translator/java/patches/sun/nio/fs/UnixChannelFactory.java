package sun.nio.fs;

import java.io.InputStream;
import java.io.OutputStream;

/**
 * The translator's patch of the JDK's {@code UnixChannelFactory} (see docs/jdk-overlays.md). The
 * JDK's wraps the descriptor it opens in a {@code FileDescriptor}, through {@code SharedSecrets},
 * for {@code sun.nio.ch.FileChannelImpl}, which reads and writes through memory the JVM gives.
 * Patched, the descriptor, one of the runtime's table, goes to the translator's
 * {@code UnixFileChannel}, or to its streams for {@code Files.newInputStream} and
 * {@code newOutputStream} ({@code LinuxFileSystemProvider}). The options and the opening are the
 * JDK's. There are no asynchronous file channels: {@code newAsynchronousFileChannel} throws
 * {@code UnsupportedOperationException}.
 */
class UnixChannelFactory {
    @Remove
    private static final JavaIOFileDescriptorAccess fdAccess;

    /**
     * The flags of a channel for {@code options}, as the JDK's {@code newFileChannel} takes them:
     * reading, unless only {@code APPEND} asks for writing, and refused when they contradict.
     */
    @Add
    private static Flags channelFlags(Set<? extends OpenOption> options) {
        Flags flags = Flags.toFlags(options);
        if (!flags.read && !flags.write) {
            flags.read = !flags.append;
            flags.write = flags.append;
        }
        if (flags.append && (flags.read || flags.truncateExisting)) {
            throw new IllegalArgumentException(
                    flags.read ? "READ + APPEND not allowed" : "APPEND + TRUNCATE_EXISTING not allowed");
        }
        return flags;
    }

    @Replace
    static FileChannel newFileChannel(int dfd, UnixPath path, Set<? extends OpenOption> options, int mode)
            throws UnixException {
        Flags flags = channelFlags(options);
        int fd = open(dfd, path, flags, mode);
        return new UnixFileChannel(fd, flags.read, flags.write, flags.append);
    }

    /** The stream of a file opened as {@link #newFileChannel} opens it, for reading. */
    @Add
    static InputStream newInputStream(UnixPath path, Set<? extends OpenOption> options, int mode)
            throws UnixException {
        return new UnixFileInputStream(open(-1, path, channelFlags(options), mode));
    }

    /** The stream of a file opened as {@link #newFileChannel} opens it, for writing. */
    @Add
    static OutputStream newOutputStream(UnixPath path, Set<? extends OpenOption> options, int mode)
            throws UnixException {
        return new UnixFileOutputStream(open(-1, path, channelFlags(options), mode));
    }

    /** There are no asynchronous file channels in a translated program. */
    @Replace
    static AsynchronousFileChannel newAsynchronousFileChannel(UnixPath path, Set<? extends OpenOption> options,
            int mode, ThreadPool pool) throws UnixException {
        throw new UnsupportedOperationException("asynchronous file channels in a translated program");
    }

    // The runtime's descriptor itself is the result, not a `FileDescriptor` around it.
    @Edit(find = "protected static FileDescriptor open(", replace = "protected static int open(")
    @Edit(from = "FileDescriptor fdObj = new FileDescriptor();", to = "return fdObj;", replace = "return fd;")
    protected static FileDescriptor open(int dfd, UnixPath path, Flags flags, int mode);
}
