package jdk.internal.jimage;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.IntBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Objects;
import java.util.stream.IntStream;

/**
 * The translator's {@code BasicImageReader}, which replaces the JDK's in a translation with the
 * JDK's sources. The JDK's maps the image file into memory ({@code FileChannel.map}), or uses the
 * map the JVM made of it, and reads its index through views of that memory. This one reads the
 * index (the header, the hash table, the locations and the strings: a few megabytes) into an
 * array once, and each resource into an array of its own when it is asked for, through the
 * file's channel, as the JDK's does where it does not map the whole file. A compressed resource
 * (an image made with {@code jlink --compress}) is refused with an
 * {@code UnsupportedOperationException}. The rest is the JDK's.
 */
public class BasicImageReader implements AutoCloseable {
    private final Path imagePath;
    private final ByteOrder byteOrder;
    private final String name;
    private final ByteBuffer memoryMap;
    private final FileChannel channel;
    private final ImageHeader header;
    private final long indexSize;
    private final IntBuffer redirect;
    private final IntBuffer offsets;
    private final ByteBuffer locations;
    private final ByteBuffer strings;
    private final ImageStringsReader stringsReader;

    @SuppressWarnings("this-escape")
    protected BasicImageReader(Path path, ByteOrder byteOrder)
            throws IOException {
        // The byte order is set first, from the parameter itself, so that the translator sees
        // that the field (an enum there) is never null.
        this.byteOrder = byteOrder;
        this.imagePath = Objects.requireNonNull(path);
        Objects.requireNonNull(byteOrder);
        this.name = this.imagePath.toString();

        channel = FileChannel.open(imagePath, StandardOpenOption.READ);

        // Read the image file header
        int headerSize = ImageHeader.getHeaderSize();
        ByteBuffer headerBuffer = ByteBuffer.allocate(headerSize);
        if (channel.read(headerBuffer, 0L) == headerSize) {
            headerBuffer.rewind();
        } else {
            throw new IOException("\"" + name + "\" is not an image file");
        }

        // Interpret the image file header
        header = readHeader(intBuffer(headerBuffer, 0, headerSize));
        indexSize = header.getIndexSize();

        // Read the image index
        ByteBuffer index = ByteBuffer.allocate((int) indexSize);
        if (readFully(index, 0L) != indexSize) {
            throw new IOException("The image file \"" + name + "\" is corrupted");
        }
        index.rewind();
        memoryMap = index.asReadOnlyBuffer();

        // Interpret the image index
        redirect = intBuffer(memoryMap, header.getRedirectOffset(), header.getRedirectSize());
        offsets = intBuffer(memoryMap, header.getOffsetsOffset(), header.getOffsetsSize());
        locations = slice(memoryMap, header.getLocationsOffset(), header.getLocationsSize());
        strings = slice(memoryMap, header.getStringsOffset(), header.getStringsSize());

        stringsReader = new ImageStringsReader(this);
    }

    /** Reads {@code buffer}'s remaining bytes from {@code position}, until the file ends. */
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

    protected BasicImageReader(Path imagePath) throws IOException {
        this(imagePath, ByteOrder.nativeOrder());
    }

    public static BasicImageReader open(Path imagePath) throws IOException {
        return new BasicImageReader(imagePath, ByteOrder.nativeOrder());
    }

    public ImageHeader getHeader() {
        return header;
    }

    private ImageHeader readHeader(IntBuffer buffer) throws IOException {
        ImageHeader result = ImageHeader.readFrom(buffer);

        if (result.getMagic() != ImageHeader.MAGIC) {
            throw new IOException("\"" + name + "\" is not an image file");
        }

        if (result.getMajorVersion() != ImageHeader.MAJOR_VERSION ||
            result.getMinorVersion() != ImageHeader.MINOR_VERSION) {
            throw new IOException("The image file \"" + name + "\" is not " +
                "the correct version. Major: " + result.getMajorVersion() +
                ". Minor: " + result.getMinorVersion());
        }

        return result;
    }

    private static ByteBuffer slice(ByteBuffer buffer, int position, int capacity) {
        // Note that this is the only limit and position manipulation of
        // BasicImageReader private ByteBuffers.  The synchronize could be avoided
        // by cloning the buffer to make a local copy, but at the cost of creating
        // a new object.
        synchronized(buffer) {
            buffer.limit(position + capacity);
            buffer.position(position);
            return buffer.slice();
        }
    }

    private IntBuffer intBuffer(ByteBuffer buffer, int offset, int size) {
        return slice(buffer, offset, size).order(byteOrder).asIntBuffer();
    }

    /** Nothing to release: every buffer is an array of its own. */
    public static void releaseByteBuffer(ByteBuffer buffer) {
        Objects.requireNonNull(buffer);
    }

    public String getName() {
        return name;
    }

    public ByteOrder getByteOrder() {
        return byteOrder;
    }

    public Path getImagePath() {
        return imagePath;
    }

    @Override
    public void close() throws IOException {
        if (channel != null) {
            channel.close();
        }
    }

    public ImageStringsReader getStrings() {
        return stringsReader;
    }

    public ImageLocation findLocation(String module, String name) {
        int index = getLocationIndex(module, name);
        if (index < 0) {
            return null;
        }
        long[] attributes = getAttributes(offsets.get(index));
        if (!ImageLocation.verify(module, name, attributes, stringsReader)) {
            return null;
        }
        return new ImageLocation(attributes, stringsReader);
    }

    public ImageLocation findLocation(String name) {
        int index = getLocationIndex(name);
        if (index < 0) {
            return null;
        }
        long[] attributes = getAttributes(offsets.get(index));
        if (!ImageLocation.verify(name, attributes, stringsReader)) {
            return null;
        }
        return new ImageLocation(attributes, stringsReader);
    }

    public boolean verifyLocation(String module, String name) {
        int index = getLocationIndex(module, name);
        if (index < 0) {
            return false;
        }
        int locationOffset = offsets.get(index);
        return ImageLocation.verify(module, name, locations, locationOffset, stringsReader);
    }

    // Details of the algorithm used here can be found in
    // jdk.tools.jlink.internal.PerfectHashBuilder.
    public int getLocationIndex(String name) {
        int count = header.getTableLength();
        int index = redirect.get(ImageStringsReader.hashCode(name) % count);
        if (index < 0) {
            // index is twos complement of location attributes index.
            return -index - 1;
        } else if (index > 0) {
            // index is hash seed needed to compute location attributes index.
            return ImageStringsReader.hashCode(name, index) % count;
        } else {
            // No entry.
            return -1;
        }
    }

    private int getLocationIndex(String module, String name) {
        int count = header.getTableLength();
        int index = redirect.get(ImageStringsReader.hashCode(module, name) % count);
        if (index < 0) {
            // index is twos complement of location attributes index.
            return -index - 1;
        } else if (index > 0) {
            // index is hash seed needed to compute location attributes index.
            return ImageStringsReader.hashCode(module, name, index) % count;
        } else {
            // No entry.
            return -1;
        }
    }

    public String[] getEntryNames() {
        return IntStream.range(0, offsets.capacity())
                .map(offsets::get)
                .filter(o -> o != 0)
                .mapToObj(o -> ImageLocation.readFrom(this, o).getFullName())
                .sorted()
                .toArray(String[]::new);
    }

    ImageLocation getLocation(int offset) {
        return ImageLocation.readFrom(this, offset);
    }

    public long[] getAttributes(int offset) {
        if (offset < 0 || offset >= locations.limit()) {
            throw new IndexOutOfBoundsException("offset");
        }
        return ImageLocation.decompress(locations, offset);
    }

    public String getString(int offset) {
        if (offset < 0 || offset >= strings.limit()) {
            throw new IndexOutOfBoundsException("offset");
        }
        return ImageStringsReader.stringFromByteBuffer(strings, offset);
    }

    public int match(int offset, String string, int stringOffset) {
        if (offset < 0 || offset >= strings.limit()) {
            throw new IndexOutOfBoundsException("offset");
        }
        return ImageStringsReader.stringFromByteBufferMatches(strings, offset, string, stringOffset);
    }

    private byte[] getBufferBytes(ByteBuffer buffer) {
        Objects.requireNonNull(buffer);
        byte[] bytes = new byte[buffer.limit()];
        buffer.get(bytes);

        return bytes;
    }

    private ByteBuffer readBuffer(long offset, long size) {
        if (offset < 0 || Integer.MAX_VALUE <= offset) {
            throw new IndexOutOfBoundsException("Bad offset: " + offset);
        }

        if (size < 0 || Integer.MAX_VALUE <= size) {
            throw new IndexOutOfBoundsException("Bad size: " + size);
        }

        ByteBuffer buffer = ByteBuffer.allocate((int) size);
        int read;
        try {
            read = readFully(buffer, offset);
            buffer.rewind();
        } catch (IOException ex) {
            throw new RuntimeException(ex);
        }

        if (read != size) {
            throw new RuntimeException("Short read: " + read +
                                       " instead of " + size + " bytes");
        }

        return buffer;
    }

    public byte[] getResource(String name) {
        Objects.requireNonNull(name);
        ImageLocation location = findLocation(name);

        return location != null ? getResource(location) : null;
    }

    public byte[] getResource(ImageLocation loc) {
        ByteBuffer buffer = getResourceBuffer(loc);

        if (buffer != null) {
            byte[] bytes = getBufferBytes(buffer);
            releaseByteBuffer(buffer);

            return bytes;
        }

        return null;
    }

    public ByteBuffer getResourceBuffer(ImageLocation loc) {
        Objects.requireNonNull(loc);
        long offset = loc.getContentOffset() + indexSize;
        long compressedSize = loc.getCompressedSize();
        long uncompressedSize = loc.getUncompressedSize();

        if (compressedSize < 0 || Integer.MAX_VALUE < compressedSize) {
            throw new IndexOutOfBoundsException(
                "Bad compressed size: " + compressedSize);
        }

        if (uncompressedSize < 0 || Integer.MAX_VALUE < uncompressedSize) {
            throw new IndexOutOfBoundsException(
                "Bad uncompressed size: " + uncompressedSize);
        }

        if (compressedSize != 0) {
            // A compressed resource (an image made with `jlink --compress`) is not read: the
            // JDK's decompressors inflate through the JVM's zip library and copy memory into
            // buffers in bulk, which a translated program does not have.
            throw new UnsupportedOperationException(
                "a compressed resource of the image \"" + name + "\" (" + loc.getFullName() + ")");
        }
        return readBuffer(offset, uncompressedSize);
    }

    public InputStream getResourceStream(ImageLocation loc) {
        Objects.requireNonNull(loc);
        byte[] bytes = getResource(loc);

        return new ByteArrayInputStream(bytes);
    }
}
