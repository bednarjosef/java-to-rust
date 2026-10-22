package sun.nio.fs;

import java.io.IOException;
import java.nio.file.Path;
import java.nio.file.spi.FileTypeDetector;
import jdk.internal.util.StaticProperty;

/**
 * The translator's {@code LinuxFileSystemProvider}, which replaces the JDK's in a translation
 * with the JDK's sources. The JDK's adds the DOS and the user-defined attribute views, which keep
 * their attributes in extended attributes, through memory of its own; this one has neither, as on
 * a file system without extended attributes: {@code getFileAttributeView} gives null for them,
 * and reading them throws {@code UnsupportedOperationException}. The rest is the JDK's.
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
}
