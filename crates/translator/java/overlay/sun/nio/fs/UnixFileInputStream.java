package sun.nio.fs;

import java.io.IOException;
import java.io.InputStream;
import java.nio.channels.ClosedChannelException;
import java.util.Objects;

/**
 * The stream of {@code Files.newInputStream} in a translation with the JDK's sources. The JDK's
 * reads a {@code FileChannel} through {@code ByteBuffer}s, whose memory the JVM gives; this one
 * reads the runtime's descriptor into the array itself ({@code UnixNativeDispatcher.readBytes}).
 * It answers as the JDK's stream on a regular file does: a closed stream throws
 * {@code ClosedChannelException}, a failed read an {@code IOException} with the system's message,
 * and {@code skip} and {@code available} go by the file's size.
 */
final class UnixFileInputStream extends InputStream {
    private final int fd;
    private boolean closed;

    UnixFileInputStream(int fd) {
        this.fd = fd;
    }

    /** A failure of a read or a write, as the JDK's channel reports it: the system's message. */
    static IOException failure(UnixException x) {
        return new IOException(x.errorString());
    }

    private void ensureOpen() throws IOException {
        if (closed) {
            throw new ClosedChannelException();
        }
    }

    @Override
    public synchronized int read() throws IOException {
        byte[] b = new byte[1];
        int n = read(b, 0, 1);
        return n == 1 ? b[0] & 0xff : -1;
    }

    @Override
    public synchronized int read(byte[] b, int off, int len) throws IOException {
        Objects.checkFromIndexSize(off, len, b.length);
        if (len == 0) {
            return 0;
        }
        ensureOpen();
        int n = UnixNativeDispatcher.readBytes(fd, b, off, len, -1);
        try {
            UnixNativeDispatcher.checkLast();
        } catch (UnixException x) {
            throw failure(x);
        }
        return n;
    }

    private long position() throws IOException {
        long position = UnixNativeDispatcher.position(fd);
        try {
            UnixNativeDispatcher.checkLast();
        } catch (UnixException x) {
            throw failure(x);
        }
        return position;
    }

    private long size() throws IOException {
        long size = UnixNativeDispatcher.size(fd);
        try {
            UnixNativeDispatcher.checkLast();
        } catch (UnixException x) {
            throw failure(x);
        }
        return size;
    }

    @Override
    public int available() throws IOException {
        ensureOpen();
        long rem = Math.max(0, size() - position());
        return (rem > Integer.MAX_VALUE) ? Integer.MAX_VALUE : (int) rem;
    }

    @Override
    public synchronized long skip(long n) throws IOException {
        ensureOpen();
        long pos = position();
        long newPos;
        if (n > 0) {
            newPos = pos + n;
            long size = size();
            if (newPos < 0 || newPos > size) {
                newPos = size;
            }
        } else {
            newPos = Long.max(pos + n, 0);
        }
        UnixNativeDispatcher.seek(fd, newPos);
        try {
            UnixNativeDispatcher.checkLast();
        } catch (UnixException x) {
            throw failure(x);
        }
        return newPos - pos;
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
            throw failure(x);
        }
    }
}
