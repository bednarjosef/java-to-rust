package java.io;

import java.nio.charset.Charset;
import java.nio.charset.CharsetDecoder;

/**
 * The translator's {@code InputStreamReader}, which replaces the JDK's in a translation with the
 * JDK's sources (see docs/natives-and-input.md, "Readers and writers over byte streams").
 *
 * <p>The JDK's class decodes through a {@code StreamDecoder}, whose {@code CharsetDecoder},
 * {@code ByteBuffer} and {@code CoderResult} the translation cannot take yet. This one reads the
 * bytes the same way, into a buffer of 8192 bytes, reading again in one call only while the stream
 * has bytes available, and decodes them with {@code new String(bytes, off, len, charset)}, which
 * the runtime does and which replaces malformed input as the JDK's decoder does. Bytes at the end
 * of the buffer that may begin a character wait for the next read. It decodes UTF-8, US-ASCII,
 * ISO-8859-1, UTF-16BE and UTF-16LE; any other charset is an
 * {@code UnsupportedOperationException} when the reader is created.
 */
public class InputStreamReader extends Reader {
    private static final int BUFFER_SIZE = 8192;

    private final InputStream in;
    private final Charset charset;
    /** 1 for the charsets of one byte per character, 2 for UTF-16, 0 for UTF-8. */
    private final int unit;
    private final byte[] bytes = new byte[BUFFER_SIZE];
    /** The bytes read and not decoded yet, at the start of {@code bytes}. */
    private int count;
    /** Characters decoded and not read yet: {@code chars[next..chars.length]}. */
    private char[] chars = new char[0];
    private int next;
    private boolean closed;

    public InputStreamReader(InputStream in) {
        this(in, Charset.defaultCharset());
    }

    public InputStreamReader(InputStream in, String charsetName) throws UnsupportedEncodingException {
        this(in, charsetNamed(charsetName));
    }

    public InputStreamReader(InputStream in, Charset cs) {
        super(in);
        this.in = in;
        this.charset = cs;
        this.unit = unitOf(cs.name());
    }

    public InputStreamReader(InputStream in, CharsetDecoder dec) {
        this(in, dec.charset());
    }

    private static Charset charsetNamed(String name) throws UnsupportedEncodingException {
        if (name == null) {
            throw new NullPointerException("charsetName");
        }
        if (!Charset.isSupported(name)) {
            throw new UnsupportedEncodingException(name);
        }
        return Charset.forName(name);
    }

    private static int unitOf(String name) {
        switch (name) {
            case "UTF-8":
                return 0;
            case "US-ASCII":
            case "ISO-8859-1":
                return 1;
            case "UTF-16BE":
            case "UTF-16LE":
                return 2;
            default:
                throw new UnsupportedOperationException("reading " + name + " text is not supported by the translation");
        }
    }

    /** The historical name of the charset, as the JDK's {@code getEncoding} gives it. */
    static String historicalName(String name) {
        switch (name) {
            case "UTF-8":
                return "UTF8";
            case "US-ASCII":
                return "ASCII";
            case "ISO-8859-1":
                return "ISO8859_1";
            case "UTF-16BE":
                return "UnicodeBigUnmarked";
            case "UTF-16LE":
                return "UnicodeLittleUnmarked";
            default:
                return name;
        }
    }

    public String getEncoding() {
        return closed ? null : historicalName(charset.name());
    }

    private void ensureOpen() throws IOException {
        if (closed) {
            throw new IOException("Stream closed");
        }
    }

    public int read() throws IOException {
        char[] one = new char[1];
        int n = read(one, 0, 1);
        return n < 0 ? -1 : one[0];
    }

    public int read(char[] cbuf, int off, int len) throws IOException {
        synchronized (lock) {
            ensureOpen();
            if (off < 0 || len < 0 || len > cbuf.length - off) {
                throw new IndexOutOfBoundsException();
            }
            if (len == 0) {
                return 0;
            }
            int n = 0;
            boolean end = false;
            while (n < len) {
                if (next < chars.length) {
                    int k = Math.min(len - n, chars.length - next);
                    System.arraycopy(chars, next, cbuf, off + n, k);
                    next += k;
                    n += k;
                    continue;
                }
                if (end || (n > 0 && !inReady())) {
                    break;
                }
                int r = in.read(bytes, count, bytes.length - count);
                if (r < 0) {
                    end = true;
                    decode(count);
                } else {
                    count += r;
                    decode(boundary());
                }
            }
            return n == 0 ? -1 : n;
        }
    }

    /** Whether the stream has bytes to read without waiting. */
    private boolean inReady() {
        try {
            return in.available() > 0;
        } catch (IOException e) {
            return false;
        }
    }

    /** Decodes the first {@code length} bytes read into the characters to read next. */
    private void decode(int length) {
        if (length == 0) {
            return;
        }
        chars = new String(bytes, 0, length, charset).toCharArray();
        next = 0;
        System.arraycopy(bytes, length, bytes, 0, count - length);
        count -= length;
    }

    /** How many of the bytes read end with a whole character: the rest may begin one. */
    private int boundary() {
        if (unit == 1) {
            return count;
        }
        if (unit == 2) {
            int whole = count - count % 2;
            if (whole >= 2) {
                boolean big = charset.name().equals("UTF-16BE");
                int high = big ? bytes[whole - 2] & 0xff : bytes[whole - 1] & 0xff;
                if (high >= 0xD8 && high <= 0xDB) {
                    whole -= 2;
                }
            }
            return whole;
        }
        for (int i = count - 1; i >= 0 && i >= count - 3; i--) {
            int b = bytes[i] & 0xff;
            if ((b & 0xC0) == 0x80) {
                continue;
            }
            int length = b >= 0xF0 ? 4 : b >= 0xE0 ? 3 : b >= 0xC0 ? 2 : 1;
            return count - i < length ? i : count;
        }
        return count;
    }

    public boolean ready() throws IOException {
        synchronized (lock) {
            ensureOpen();
            return next < chars.length || count > 0 || inReady();
        }
    }

    public void close() throws IOException {
        synchronized (lock) {
            if (closed) {
                return;
            }
            try {
                in.close();
            } finally {
                closed = true;
            }
        }
    }
}
