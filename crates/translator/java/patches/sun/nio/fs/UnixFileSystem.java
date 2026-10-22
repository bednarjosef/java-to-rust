package sun.nio.fs;

/**
 * The translator's patch of the JDK's {@code UnixFileSystem}, the copy that {@code Files.copy}
 * makes through a buffer (see docs/natives-and-input.md). The JDK's native throws the
 * {@code UnixException} of a failed read or write; here it answers the {@code errno} of that
 * failure (0 where it succeeded), and {@link #bufferedCopy} throws the exception with it, as
 * HotSpot does, so {@code copyFile} turns it into the same {@code IOException} subclass.
 */
class UnixFileSystem {
    /**
     * Copies data between the descriptors through the buffer at {@code address}; answers the
     * {@code errno} of a failed read or write, 0 where the copy succeeded.
     */
    @Add
    private static native int bufferedCopy0Errno(int dst, int src, long address,
                                                 int size, long addressToPollForCancel);

    @Replace
    void bufferedCopy(int dst, int src, long address,
                      int size, long addressToPollForCancel)
        throws UnixException
    {
        int errno = bufferedCopy0Errno(dst, src, address, size, addressToPollForCancel);
        if (errno != 0)
            throw new UnixException(errno);
    }

    @Remove
    private static native void bufferedCopy0(int dst, int src, long address,
                                             int size, long addressToPollForCancel)
        throws UnixException;
}
