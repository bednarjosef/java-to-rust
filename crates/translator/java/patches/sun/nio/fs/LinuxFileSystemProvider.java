package sun.nio.fs;

import java.io.InputStream;
import java.io.OutputStream;
import java.util.HashSet;
import java.util.Set;

/**
 * The translator's patch of the JDK's {@code LinuxFileSystemProvider} (see docs/jdk-overlays.md).
 * Its streams of a file read and write the runtime's descriptor themselves
 * ({@code UnixFileInputStream}, {@code UnixFileOutputStream}), where the JDK's go through a
 * channel and {@code ByteBuffer}s; they take the options as the JDK's do. The DOS and the
 * user-defined attribute views are the JDK's, over the runtime's extended attributes.
 */
class LinuxFileSystemProvider {
    /** A stream that reads the file: any option is taken but {@code APPEND} and {@code WRITE}. */
    @Add
    @Override
    public InputStream newInputStream(Path obj, OpenOption... options) throws IOException {
        Set<OpenOption> opts = new HashSet<>();
        for (OpenOption option : options) {
            if (option == StandardOpenOption.APPEND || option == StandardOpenOption.WRITE) {
                throw new UnsupportedOperationException("'" + option + "' not allowed");
            }
            opts.add(option);
        }
        UnixPath file = UnixPath.toUnixPath(obj);
        try {
            return UnixChannelFactory.newInputStream(file, opts, UnixFileModeAttribute.ALL_READWRITE);
        } catch (UnixException x) {
            x.rethrowAsIOException(file);
            return null; // not reached: the line above throws
        }
    }

    /**
     * A stream that writes the file: any option is taken but {@code READ}, with {@code WRITE};
     * without options, the file is created or truncated.
     */
    @Add
    @Override
    public OutputStream newOutputStream(Path obj, OpenOption... options) throws IOException {
        Set<OpenOption> opts = new HashSet<>();
        for (OpenOption option : options) {
            if (option == StandardOpenOption.READ) {
                throw new IllegalArgumentException("READ not allowed");
            }
            opts.add(option);
        }
        if (options.length == 0) {
            opts.add(StandardOpenOption.CREATE);
            opts.add(StandardOpenOption.TRUNCATE_EXISTING);
        }
        opts.add(StandardOpenOption.WRITE);
        UnixPath file = UnixPath.toUnixPath(obj);
        try {
            return UnixChannelFactory.newOutputStream(file, opts, UnixFileModeAttribute.ALL_READWRITE);
        } catch (UnixException x) {
            x.rethrowAsIOException(file);
            return null; // not reached: the line above throws
        }
    }
}
