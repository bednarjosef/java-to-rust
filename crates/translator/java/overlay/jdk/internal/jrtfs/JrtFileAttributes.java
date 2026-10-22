package jdk.internal.jrtfs;

import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.FileTime;
import jdk.internal.jimage.ImageReader.Node;

/**
 * The translator's {@code JrtFileAttributes}, which replaces the JDK's in a translation with the
 * JDK's sources. It is the JDK's, except that {@code toString} throws an
 * {@code UnsupportedOperationException}: the JDK's formats the times with {@code Formatter}'s
 * {@code %tc}, which needs the calendars and time zones that a translated program does not have
 * yet. It is never called by the file system itself.
 *
 * File attributes implementation for jrt image file system.
 *
 * @implNote This class needs to maintain JDK 8 source compatibility.
 *
 * It is used internally in the JDK to implement jimage/jrtfs access,
 * but also compiled and delivered as part of the jrtfs.jar to support access
 * to the jimage file provided by the shipped JDK by tools running on JDK 8.
 */
final class JrtFileAttributes  implements BasicFileAttributes {

    private final Node node;

    JrtFileAttributes(Node node) {
        this.node = node;
    }

    //-------- basic attributes --------
    @Override
    public FileTime creationTime() {
        return node.getFileAttributes().creationTime();
    }

    @Override
    public boolean isDirectory() {
        return node.isDirectory();
    }

    @Override
    public boolean isOther() {
        return false;
    }

    @Override
    public boolean isRegularFile() {
        return !isDirectory();
    }

    @Override
    public FileTime lastAccessTime() {
        return node.getFileAttributes().lastAccessTime();
    }

    @Override
    public FileTime lastModifiedTime() {
        return node.getFileAttributes().lastModifiedTime();
    }

    @Override
    public long size() {
        return node.size();
    }

    @Override
    public boolean isSymbolicLink() {
        return node.isLink();
    }

    @Override
    public Object fileKey() {
        return node.resolveLink(true);
    }

    //-------- jrtfs specific attributes --------
    /**
     * Compressed resource file. If not available or not applicable, 0L is
     * returned.
     *
     * @return the compressed resource size for compressed resources.
     */
    public long compressedSize() {
        return node.compressedSize();
    }

    /**
     * "file" extension of a file resource.
     *
     * @return extension string for the file resource
     */
    public String extension() {
        return node.extension();
    }

    /**
     * Not in a translated program: the JDK's formats the times with {@code Formatter}'s
     * {@code %tc}, whose calendars and time zones a translated program does not have yet.
     */
    @Override
    public final String toString() {
        throw new UnsupportedOperationException("JrtFileAttributes.toString, which formats times with %tc,");
    }
}
