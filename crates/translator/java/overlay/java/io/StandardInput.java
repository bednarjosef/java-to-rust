package java.io;

/**
 * {@code System.in} as an object, for a translated program that reads it: the translator reads
 * {@code System.in} as {@link #in}. It is the object the JVM makes, a
 * {@code BufferedInputStream} over a {@code FileInputStream} of descriptor 0, both translated
 * from the JDK, whose natives are the runtime's descriptor table (`java_runtime::fd`).
 */
final class StandardInput {
    static final InputStream in = new BufferedInputStream(new FileInputStream(FileDescriptor.in));

    private StandardInput() {
    }
}
