package sun.nio.fs;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.MappedByteBuffer;
import java.nio.channels.ClosedChannelException;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.NonReadableChannelException;
import java.nio.channels.NonWritableChannelException;
import java.nio.channels.ReadableByteChannel;
import java.nio.channels.WritableByteChannel;
import java.util.Objects;

/**
 * The {@code FileChannel} of {@code Files.newByteChannel} and {@code FileChannel.open} in a
 * translation with the JDK's sources, on a descriptor of the runtime's table
 * ({@code UnixChannelFactory}). The JDK's {@code sun.nio.ch.FileChannelImpl} reads and writes
 * through memory the JVM gives; this one copies between the buffer and an array that the
 * runtime reads into or writes from ({@code UnixNativeDispatcher.readBytes}, {@code writeBytes}).
 * A failed call throws an {@code IOException} with the system's message, as the JDK's does.
 * Neither memory-mapped files nor file locks are here: {@code map}, {@code lock} and
 * {@code tryLock} throw {@code UnsupportedOperationException}. A thread interrupted in a call
 * does not close the channel.
 */
final class UnixFileChannel extends FileChannel {
    /** The most that one call reads or writes: a read may give fewer bytes than asked. */
    private static final int CHUNK = 1 << 20;

    private final int fd;
    private final boolean readable;
    private final boolean writable;
    private final boolean append;

    UnixFileChannel(int fd, boolean readable, boolean writable, boolean append) {
        this.fd = fd;
        this.readable = readable;
        this.writable = writable;
        this.append = append;
    }

    private void ensureOpen() throws IOException {
        if (!isOpen()) {
            throw new ClosedChannelException();
        }
    }

    private void ensureReadable() throws IOException {
        ensureOpen();
        if (!readable) {
            throw new NonReadableChannelException();
        }
    }

    private void ensureWritable() throws IOException {
        ensureOpen();
        if (!writable) {
            throw new NonWritableChannelException();
        }
    }

    private static void check() throws IOException {
        try {
            UnixNativeDispatcher.checkLast();
        } catch (UnixException x) {
            throw UnixFileInputStream.failure(x);
        }
    }

    /** Reads into {@code dst} at {@code position}, or at the channel's position where it is -1. */
    private int readAt(ByteBuffer dst, long position) throws IOException {
        int len = Math.min(dst.remaining(), CHUNK);
        if (len == 0) {
            return 0;
        }
        byte[] buf = new byte[len];
        int n = UnixNativeDispatcher.readBytes(fd, buf, 0, len, position);
        check();
        if (n > 0) {
            dst.put(buf, 0, n);
        }
        return n;
    }

    /** Writes all of {@code src} at {@code position}, or at the channel's position where it is -1. */
    private int writeAt(ByteBuffer src, long position) throws IOException {
        int written = 0;
        while (src.hasRemaining()) {
            int len = Math.min(src.remaining(), CHUNK);
            byte[] buf = new byte[len];
            src.get(buf, 0, len);
            int done = 0;
            while (done < len) {
                long at = position < 0 ? -1 : position + written + done;
                int n = UnixNativeDispatcher.writeBytes(fd, buf, done, len - done, at);
                check();
                done += n;
            }
            written += len;
        }
        return written;
    }

    @Override
    public synchronized int read(ByteBuffer dst) throws IOException {
        Objects.requireNonNull(dst);
        ensureReadable();
        return readAt(dst, -1);
    }

    @Override
    public synchronized long read(ByteBuffer[] dsts, int offset, int length) throws IOException {
        Objects.checkFromIndexSize(offset, length, dsts.length);
        ensureReadable();
        long total = 0;
        for (int i = offset; i < offset + length; i++) {
            ByteBuffer dst = dsts[i];
            int asked = Math.min(dst.remaining(), CHUNK);
            int n = readAt(dst, -1);
            if (n < 0) {
                return total == 0 ? -1 : total;
            }
            total += n;
            if (n < asked) {
                break;
            }
        }
        return total;
    }

    @Override
    public synchronized int write(ByteBuffer src) throws IOException {
        Objects.requireNonNull(src);
        ensureWritable();
        return writeAt(src, -1);
    }

    @Override
    public synchronized long write(ByteBuffer[] srcs, int offset, int length) throws IOException {
        Objects.checkFromIndexSize(offset, length, srcs.length);
        ensureWritable();
        long total = 0;
        for (int i = offset; i < offset + length; i++) {
            total += writeAt(srcs[i], -1);
        }
        return total;
    }

    @Override
    public synchronized long position() throws IOException {
        ensureOpen();
        long position = append ? UnixNativeDispatcher.size(fd) : UnixNativeDispatcher.position(fd);
        check();
        return position;
    }

    @Override
    public synchronized FileChannel position(long newPosition) throws IOException {
        if (newPosition < 0) {
            throw new IllegalArgumentException();
        }
        ensureOpen();
        UnixNativeDispatcher.seek(fd, newPosition);
        check();
        return this;
    }

    @Override
    public synchronized long size() throws IOException {
        ensureOpen();
        long size = UnixNativeDispatcher.size(fd);
        check();
        return size;
    }

    @Override
    public synchronized FileChannel truncate(long newSize) throws IOException {
        if (newSize < 0) {
            throw new IllegalArgumentException("Negative size");
        }
        ensureWritable();
        long position = position();
        if (newSize < size()) {
            UnixNativeDispatcher.truncate(fd, newSize);
            check();
        }
        if (position > newSize) {
            position(newSize);
        }
        return this;
    }

    @Override
    public void force(boolean metaData) throws IOException {
        ensureOpen();
        UnixNativeDispatcher.sync(fd, metaData);
        check();
    }

    @Override
    public long transferTo(long position, long count, WritableByteChannel target) throws IOException {
        ensureReadable();
        Objects.requireNonNull(target);
        if (!target.isOpen()) {
            throw new ClosedChannelException();
        }
        if (position < 0 || count < 0) {
            throw new IllegalArgumentException();
        }
        long done = 0;
        while (done < count) {
            ByteBuffer buf = ByteBuffer.allocate((int) Math.min(count - done, 8192));
            int n = readAt(buf, position + done);
            if (n <= 0) {
                break;
            }
            buf.flip();
            while (buf.hasRemaining()) {
                target.write(buf);
            }
            done += n;
        }
        return done;
    }

    @Override
    public long transferFrom(ReadableByteChannel src, long position, long count) throws IOException {
        ensureWritable();
        Objects.requireNonNull(src);
        if (!src.isOpen()) {
            throw new ClosedChannelException();
        }
        if (position < 0 || count < 0) {
            throw new IllegalArgumentException();
        }
        if (position > size()) {
            return 0;
        }
        long done = 0;
        while (done < count) {
            ByteBuffer buf = ByteBuffer.allocate((int) Math.min(count - done, 8192));
            int n = src.read(buf);
            if (n <= 0) {
                break;
            }
            buf.flip();
            writeAt(buf, position + done);
            done += n;
        }
        return done;
    }

    @Override
    public synchronized int read(ByteBuffer dst, long position) throws IOException {
        Objects.requireNonNull(dst);
        if (position < 0) {
            throw new IllegalArgumentException("Negative position");
        }
        ensureReadable();
        return readAt(dst, position);
    }

    @Override
    public synchronized int write(ByteBuffer src, long position) throws IOException {
        Objects.requireNonNull(src);
        if (position < 0) {
            throw new IllegalArgumentException("Negative position");
        }
        ensureWritable();
        return writeAt(src, position);
    }

    @Override
    public MappedByteBuffer map(MapMode mode, long position, long size) throws IOException {
        throw new UnsupportedOperationException("memory-mapped files in a translated program");
    }

    @Override
    public FileLock lock(long position, long size, boolean shared) throws IOException {
        throw new UnsupportedOperationException("file locks in a translated program");
    }

    @Override
    public FileLock tryLock(long position, long size, boolean shared) throws IOException {
        throw new UnsupportedOperationException("file locks in a translated program");
    }

    @Override
    protected void implCloseChannel() throws IOException {
        try {
            UnixNativeDispatcher.close(fd);
        } catch (UnixException x) {
            throw UnixFileInputStream.failure(x);
        }
    }
}
