package sun.nio.fs;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.OpenOption;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.spi.FileTypeDetector;
import java.util.Collections;
import java.util.HashSet;
import java.util.Set;
import jdk.internal.util.StaticProperty;

/**
 * The translator's {@code LinuxFileSystemProvider}, which replaces the JDK's in a translation
 * with the JDK's sources. The JDK's adds the DOS and the user-defined attribute views, which keep
 * their attributes in extended attributes, through memory of its own; this one has neither, as on
 * a file system without extended attributes: {@code getFileAttributeView} gives null for them,
 * and reading them throws {@code UnsupportedOperationException}. Its streams of a file read and
 * write the runtime's descriptor themselves ({@code UnixFileInputStream},
 * {@code UnixFileOutputStream}), where the JDK's go through a channel and {@code ByteBuffer}s;
 * they take the options as the JDK's do. The rest is the JDK's.
 */
class LinuxFileSystemProvider extends UnixFileSystemProvider {
    public LinuxFileSystemProvider() {
        super();
    }

    @Override
    LinuxFileSystem newFileSystem(String dir) {
        return new LinuxFileSystem(this, dir);
    }

    @Override
    LinuxFileStore getFileStore(UnixPath path) throws IOException {
        return new LinuxFileStore(path);
    }

    @Override
    FileTypeDetector getFileTypeDetector() {
        String userHome = StaticProperty.userHome();
        Path userMimeTypes = Path.of(userHome, ".mime.types");
        Path etcMimeTypes = Path.of("/etc/mime.types");
        return chain(new MimeTypesFileTypeDetector(userMimeTypes), new MimeTypesFileTypeDetector(etcMimeTypes));
    }

    @Override
    public InputStream newInputStream(Path obj, OpenOption... options) throws IOException {
        for (OpenOption opt : options) {
            // All OpenOption values except for APPEND and WRITE are allowed
            if (opt == StandardOpenOption.APPEND || opt == StandardOpenOption.WRITE)
                throw new UnsupportedOperationException("'" + opt + "' not allowed");
        }
        Set<OpenOption> opts = new HashSet<>();
        Collections.addAll(opts, options);
        UnixPath file = UnixPath.toUnixPath(obj);
        try {
            return UnixChannelFactory.newInputStream(file, opts, UnixFileModeAttribute.ALL_READWRITE);
        } catch (UnixException x) {
            x.rethrowAsIOException(file);
            return null;  // keep compiler happy
        }
    }

    @Override
    public OutputStream newOutputStream(Path obj, OpenOption... options) throws IOException {
        Set<OpenOption> opts = new HashSet<>();
        if (options.length == 0) {
            opts.add(StandardOpenOption.CREATE);
            opts.add(StandardOpenOption.TRUNCATE_EXISTING);
        } else {
            for (OpenOption opt : options) {
                if (opt == StandardOpenOption.READ)
                    throw new IllegalArgumentException("READ not allowed");
                opts.add(opt);
            }
        }
        opts.add(StandardOpenOption.WRITE);
        UnixPath file = UnixPath.toUnixPath(obj);
        try {
            return UnixChannelFactory.newOutputStream(file, opts, UnixFileModeAttribute.ALL_READWRITE);
        } catch (UnixException x) {
            x.rethrowAsIOException(file);
            return null;  // keep compiler happy
        }
    }
}
