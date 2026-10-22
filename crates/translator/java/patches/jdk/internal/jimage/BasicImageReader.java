package jdk.internal.jimage;

/**
 * The translator's patch of the JDK's {@code BasicImageReader} (see docs/jdk-overlays.md). The
 * JDK's maps the image file into memory ({@code FileChannel.map}), or uses the map the JVM made of
 * it, and reads its index through views of that memory. Patched, it reads the index (the header,
 * the hash table, the locations and the strings: a few megabytes) into an array once, and each
 * resource into an array of its own when it is asked for, through the file's channel, as the
 * JDK's does where it does not map the whole file. A compressed resource (an image made with
 * {@code jlink --compress}) is refused with an {@code UnsupportedOperationException}.
 */
class BasicImageReader {
    // The byte order is set first, from the parameter itself, so that the translator sees that
    // the field (an enum there) is never null.
    @Edit(from = "this.imagePath =", to = "this.byteOrder = Objects.requireNonNull(byteOrder);", replace = """
            this.byteOrder = byteOrder;
            this.imagePath = Objects.requireNonNull(path);
            Objects.requireNonNull(byteOrder);
            """)
    // No map of the JVM's, no map of the whole file, and no reflection to make the channel
    // uninterruptible: the file is opened and its header read into an array.
    @Edit(from = "ByteBuffer map;", to = "ByteBuffer headerBuffer = map;",
            replace = "channel = FileChannel.open(imagePath, StandardOpenOption.READ);")
    @Edit(from = "if (headerBuffer == null) {", to = "headerBuffer = ByteBuffer.allocateDirect(headerSize);",
            replace = "ByteBuffer headerBuffer = ByteBuffer.allocate(headerSize);")
    @Edit(from = "} else if (headerBuffer.capacity() < headerSize) {", to = "}", replace = "")
    // The index is read into an array of its own.
    @Edit(from = "if (map == null) {", to = "memoryMap = map.asReadOnlyBuffer();", replace = """
            ByteBuffer index = ByteBuffer.allocate((int) indexSize);
            if (readFully(index, 0L) != indexSize) {
                throw new IOException("The image file \\"" + name + "\\" is corrupted");
            }
            index.rewind();
            memoryMap = index.asReadOnlyBuffer();
            """)
    @Edit(from = "if (memoryMap.capacity() < indexSize) {", to = "}", replace = "")
    @Edit(find = "decompressor = new Decompressor();", replace = "")
    protected BasicImageReader(Path path, ByteOrder byteOrder);

    /** Reads {@code buffer}'s remaining bytes from {@code position}, until the file ends. */
    @Add
    private int readFully(ByteBuffer buffer, long position) throws IOException {
        int total = 0;
        while (buffer.hasRemaining()) {
            int n = channel.read(buffer, position + total);
            if (n < 0) {
                break;
            }
            total += n;
        }
        return total;
    }

    // Nothing to release: every buffer is an array of its own.
    @Edit(from = "if (!MAP_ALL) {", to = "}", replace = "")
    public static void releaseByteBuffer(ByteBuffer buffer);

    // A resource is read into an array of its own, through the channel, until the file ends.
    @Edit(from = "if (MAP_ALL) {", to = "ByteBuffer buffer = ImageBufferCache.getBuffer(size);",
            replace = "ByteBuffer buffer = ByteBuffer.allocate((int) size);")
    @Edit(find = "channel.read(buffer, offset)", replace = "readFully(buffer, offset)")
    @Edit(from = "} catch (IOException ex) {", to = "ImageBufferCache.releaseBuffer(buffer);",
            replace = "} catch (IOException ex) {")
    @Edit(from = "if (read != size) {", to = "ImageBufferCache.releaseBuffer(buffer);", replace = "if (read != size) {")
    @Edit(from = "\" bytes\");", to = "return buffer; }", replace = """
            " bytes");
            }

            return buffer;
            """)
    private ByteBuffer readBuffer(long offset, long size);

    @Edit(find = "ImageBufferCache.releaseBuffer(buffer);", replace = "releaseByteBuffer(buffer);")
    public byte[] getResource(ImageLocation loc);

    // A compressed resource (an image made with `jlink --compress`) is not read: the JDK's
    // decompressors inflate through the JVM's zip library and copy memory into buffers in bulk,
    // which a translated program does not have.
    @Edit(from = "if (compressedSize == 0) {", to = "return null;", replace = """
            if (compressedSize != 0) {
                throw new UnsupportedOperationException(
                    "a compressed resource of the image \\"" + name + "\\" (" + loc.getFullName() + ")");
            }
            return readBuffer(offset, uncompressedSize);
            """)
    public ByteBuffer getResourceBuffer(ImageLocation loc);

    // The properties that choose how the JDK's maps the file, read through `AccessController`.
    @Remove
    private static boolean isSystemProperty(String key, String value, String def);

    @Remove
    private static final boolean IS_64_BIT;

    @Remove
    private static final boolean USE_JVM_MAP;

    @Remove
    private static final boolean MAP_ALL;

    @Remove
    private final Decompressor decompressor;
}
