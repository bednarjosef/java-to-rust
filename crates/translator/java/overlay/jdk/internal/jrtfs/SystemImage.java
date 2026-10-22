package jdk.internal.jrtfs;

import java.io.IOException;
import java.nio.file.FileSystem;
import java.nio.file.FileSystemNotFoundException;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import jdk.internal.jimage.ImageReader;
import jdk.internal.jimage.ImageReader.Node;

/**
 * The translator's {@code SystemImage}, which replaces the JDK's in a translation with the JDK's
 * sources: the class library behind the {@code jrt:/} file system. The JDK's finds the JDK's home
 * from where its own class was loaded (a {@code CodeSource}, through {@code AccessController}),
 * which for the boot loader is the {@code java.home} property; this one reads that property,
 * which a translated program answers when it runs (`lower/system.rs`). A JDK without
 * {@code lib/modules} (its exploded modules, as it is built) is refused with an
 * {@code UnsupportedOperationException}. The rest is the JDK's.
 */
abstract class SystemImage {

    abstract Node findNode(String path) throws IOException;
    abstract byte[] getResource(Node node) throws IOException;
    abstract void close() throws IOException;

    static SystemImage open() throws IOException {
        if (modulesImageExists) {
            // open a .jimage and build directory structure
            final ImageReader image = ImageReader.open(moduleImageFile);
            return new SystemImage() {
                @Override
                Node findNode(String path) throws IOException {
                    return image.findNode(path);
                }
                @Override
                byte[] getResource(Node node) throws IOException {
                    return image.getResource(node);
                }
                @Override
                void close() throws IOException {
                    image.close();
                }
            };
        }
        if (Files.notExists(explodedModulesDir))
            throw new FileSystemNotFoundException(explodedModulesDir.toString());
        // The modules of a JDK as it is built, a directory of each module's classes, are not
        // read: the JDK's reader of them is not translated.
        throw new UnsupportedOperationException("the exploded modules of " + explodedModulesDir);
    }

    private static final String RUNTIME_HOME;
    // "modules" jimage file Path
    private static final Path moduleImageFile;
    // "modules" jimage exists or not?
    private static final boolean modulesImageExists;
    // <JAVA_HOME>/modules directory Path
    private static final Path explodedModulesDir;

    static {
        RUNTIME_HOME = System.getProperty("java.home");

        FileSystem fs = FileSystems.getDefault();
        moduleImageFile = fs.getPath(RUNTIME_HOME, "lib", "modules");
        explodedModulesDir = fs.getPath(RUNTIME_HOME, "modules");

        modulesImageExists = Files.isRegularFile(moduleImageFile);
    }
}
