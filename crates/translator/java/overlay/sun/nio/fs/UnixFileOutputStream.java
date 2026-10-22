package sun.nio.fs;

import java.io.IOException;
import java.io.OutputStream;
import java.nio.channels.ClosedChannelException;
import java.util.Objects;

/**
 * The stream of {@code Files.newOutputStream} in a translation with the JDK's sources, which
 * writes the array to the runtime's descriptor itself, where the JDK's writes a
 * {@code FileChannel} through {@code ByteBuffer}s (see {@code UnixFileInputStream}). A write
 * writes every byte, as the JDK's does.
 */
final class UnixFileOutputStream extends OutputStream {
    private final int fd;
    private boolean closed;

    UnixFileOutputStream(int fd) {
        this.fd = fd;
    }

    private void ensureOpen() throws IOException {
        if (closed) {
            throw new ClosedChannelException();
        }
    }

    @Override
    public synchronized void write(int b) throws IOException {
        write(new byte[] {(byte) b}, 0, 1);
    }

    @Override
    public synchronized void write(byte[] b, int off, int len) throws IOException {
        Objects.checkFromIndexSize(off, len, b.length);
        if (len == 0) {
            return;
        }
        ensureOpen();
        int done = 0;
        while (done < len) {
            int n = UnixNativeDispatcher.writeBytes(fd, b, off + done, len - done, -1);
            try {
                UnixNativeDispatcher.checkLast();
            } catch (UnixException x) {
                throw UnixFileInputStream.failure(x);
            }
            done += n;
        }
    }

    @Override
    public synchronized void close() throws IOException {
        if (closed) {
            return;
        }
        closed = true;
        try {
            UnixNativeDispatcher.close(fd);
        } catch (UnixException x) {
            throw UnixFileInputStream.failure(x);
        }
    }
}
