package sun.nio.fs;

/**
 * The translator's patch of the JDK's {@code UnixFileStoreAttributes}, the {@code statvfs} of a
 * file system (see docs/jdk-overlays.md). The JDK's native writes its fields; here the translator's
 * {@code UnixNativeDispatcher} gives them as an array, which {@link #fill} copies.
 */
class UnixFileStoreAttributes {
    /** Copies the fields that {@code UnixNativeDispatcher.statvfs} gives, in its order. */
    @Add
    void fill(long[] f) {
        f_frsize = f[0];
        f_blocks = f[1];
        f_bfree = f[2];
        f_bavail = f[3];
    }
}
