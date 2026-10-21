package java.io;

/**
 * {@code System.out} and {@code System.err} as objects, for a translated program that uses them
 * as values: the translator reads {@code System.out} as {@link #out} and {@code System.err} as
 * {@link #err} (see docs/natives-and-input.md, "System.out, System.err and System.in as values").
 *
 * <p>The JVM makes each a {@code PrintStream} with automatic flushing over a
 * {@code BufferedOutputStream} of 128 bytes over a {@code FileOutputStream} of its descriptor.
 * Here the stream under the {@code PrintStream} is this class, whose natives are the runtime's
 * standard output and error (`java_runtime::out`): the runtime keeps that buffer itself, because
 * the calls that it translates directly ({@code System.out.println(x)} and the like) write
 * through it too, and both must stay in order.
 */
final class StandardOutput extends OutputStream {
    static final PrintStream out = new PrintStream(new StandardOutput(1), true);
    static final PrintStream err = new PrintStream(new StandardOutput(2), true);

    /** 1 for standard output, 2 for standard error. */
    private final int fd;

    private StandardOutput(int fd) {
        this.fd = fd;
    }

    @Override
    public void write(int b) {
        write0(fd, b);
    }

    @Override
    public void write(byte[] b, int off, int len) {
        if (off < 0 || len < 0 || off > b.length - len) {
            // What the JVM's stream throws for a range outside the array: the buffer of 128 bytes
            // copies a shorter range with `System.arraycopy`, and a longer one goes to the file
            // after the buffer is written out, whose native throws without a message.
            if (len < BUFFER) {
                System.arraycopy(b, off, new byte[BUFFER], 0, len);
            }
            flush0(fd);
            throw new IndexOutOfBoundsException();
        }
        writeBytes0(fd, b, off, len);
    }

    /** The size of the JVM's buffer under {@code System.out} and {@code System.err}. */
    private static final int BUFFER = 128;

    @Override
    public void flush() {
        flush0(fd);
    }

    @Override
    public void close() {
        close0(fd);
    }

    private static native void write0(int fd, int b);

    private static native void writeBytes0(int fd, byte[] b, int off, int len);

    private static native void flush0(int fd);

    private static native void close0(int fd);
}
