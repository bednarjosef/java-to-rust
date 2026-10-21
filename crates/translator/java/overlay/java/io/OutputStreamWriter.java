package java.io;

import java.nio.charset.Charset;
import java.nio.charset.CharsetEncoder;

/**
 * The translator's {@code OutputStreamWriter}, which replaces the JDK's in a translation with the
 * JDK's sources (see docs/natives-and-input.md, "Readers and writers over byte streams").
 *
 * <p>The JDK's class encodes through a {@code StreamEncoder}, whose {@code CharsetEncoder},
 * {@code ByteBuffer} and {@code CoderResult} the translation cannot take yet. This one encodes
 * each character itself into a buffer that starts at 512 bytes and grows to 8192 for a longer
 * write, and passes the buffer to the stream when the next character does not fit, on
 * {@code flush} and on {@code close}, as the JDK's encoder does: the same bytes reach the stream
 * in the same calls, and what is still buffered when the program ends is lost in both. A lone
 * surrogate is written as the charset's replacement ({@code ?}, or U+FFFD in UTF-16), and so is a
 * character that the charset has no byte for. It
 * encodes UTF-8, US-ASCII, ISO-8859-1, UTF-16BE and UTF-16LE; any other charset is an
 * {@code UnsupportedOperationException} when the writer is created.
 */
public class OutputStreamWriter extends Writer {
    private static final int INITIAL_CAPACITY = 512;
    private static final int MAX_CAPACITY = 8192;

    private final OutputStream out;
    private final Charset charset;
    /** 0 for UTF-8, 1 for US-ASCII, 2 for ISO-8859-1, 3 for UTF-16BE, 4 for UTF-16LE. */
    private final int kind;
    private byte[] buf = new byte[INITIAL_CAPACITY];
    private int count;
    /** A high surrogate that ended the last write, waiting for its pair; -1 for none. */
    private int leftover = -1;
    private boolean closed;

    public OutputStreamWriter(OutputStream out, String charsetName) throws UnsupportedEncodingException {
        this(out, charsetNamed(charsetName));
    }

    public OutputStreamWriter(OutputStream out) {
        this(out, out instanceof PrintStream ps ? ps.charset() : Charset.defaultCharset());
    }

    public OutputStreamWriter(OutputStream out, Charset cs) {
        super(out);
        this.out = out;
        this.charset = cs;
        this.kind = kindOf(cs.name());
    }

    public OutputStreamWriter(OutputStream out, CharsetEncoder enc) {
        this(out, enc.charset());
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

    private static int kindOf(String name) {
        switch (name) {
            case "UTF-8":
                return 0;
            case "US-ASCII":
                return 1;
            case "ISO-8859-1":
                return 2;
            case "UTF-16BE":
                return 3;
            case "UTF-16LE":
                return 4;
            default:
                throw new UnsupportedOperationException("writing " + name + " text is not supported by the translation");
        }
    }

    public String getEncoding() {
        return closed ? null : InputStreamReader.historicalName(charset.name());
    }

    private void ensureOpen() throws IOException {
        if (closed) {
            throw new IOException("Stream closed");
        }
    }

    /** Passes the buffered bytes on to the stream. */
    private void writeBytes() throws IOException {
        if (count > 0) {
            out.write(buf, 0, count);
            count = 0;
        }
    }

    void flushBuffer() throws IOException {
        synchronized (lock) {
            ensureOpen();
            writeBytes();
        }
    }

    public void write(int c) throws IOException {
        write(new char[] {(char) c}, 0, 1);
    }

    public void write(char[] cbuf, int off, int len) throws IOException {
        synchronized (lock) {
            ensureOpen();
            if (off < 0 || off > cbuf.length || len < 0 || off + len > cbuf.length || off + len < 0) {
                throw new IndexOutOfBoundsException();
            }
            if (len == 0) {
                return;
            }
            int i = off;
            int end = off + len;
            if (leftover >= 0) {
                char high = (char) leftover;
                leftover = -1;
                if (Character.isLowSurrogate(cbuf[i])) {
                    codePoint(Character.toCodePoint(high, cbuf[i]));
                    i++;
                } else {
                    replacement();
                }
            }
            grow(end - i);
            while (i < end) {
                char c = cbuf[i];
                if (Character.isHighSurrogate(c)) {
                    if (i + 1 == end) {
                        leftover = c;
                        i++;
                    } else if (Character.isLowSurrogate(cbuf[i + 1])) {
                        codePoint(Character.toCodePoint(c, cbuf[i + 1]));
                        i += 2;
                    } else {
                        replacement();
                        i++;
                    }
                } else if (Character.isLowSurrogate(c)) {
                    replacement();
                    i++;
                } else {
                    codePoint(c);
                    i++;
                }
            }
        }
    }

    /** A longer write gets a larger buffer, up to 8192 bytes; what was buffered goes on first. */
    private void grow(int chars) throws IOException {
        if (buf.length >= MAX_CAPACITY) {
            return;
        }
        int perChar = kind == 0 ? 3 : kind >= 3 ? 2 : 1;
        int capacity = (int) Math.min((long) chars * perChar, MAX_CAPACITY);
        if (capacity > buf.length) {
            writeBytes();
            buf = new byte[capacity];
        }
    }

    /** Room for {@code n} more bytes, passing the buffer on if they do not fit. */
    private void room(int n) throws IOException {
        if (count + n > buf.length) {
            writeBytes();
        }
    }

    /** What the charset writes for a malformed or unmappable character. */
    private void replacement() throws IOException {
        if (kind >= 3) {
            unit(0xFFFD);
        } else {
            room(1);
            buf[count++] = (byte) '?';
        }
    }

    private void unit(int u) throws IOException {
        room(2);
        if (kind == 3) {
            buf[count++] = (byte) (u >> 8);
            buf[count++] = (byte) u;
        } else {
            buf[count++] = (byte) u;
            buf[count++] = (byte) (u >> 8);
        }
    }

    private void codePoint(int cp) throws IOException {
        switch (kind) {
            case 0 -> {
                if (cp < 0x80) {
                    room(1);
                    buf[count++] = (byte) cp;
                } else if (cp < 0x800) {
                    room(2);
                    buf[count++] = (byte) (0xC0 | (cp >> 6));
                    buf[count++] = (byte) (0x80 | (cp & 0x3F));
                } else if (cp < 0x10000) {
                    room(3);
                    buf[count++] = (byte) (0xE0 | (cp >> 12));
                    buf[count++] = (byte) (0x80 | ((cp >> 6) & 0x3F));
                    buf[count++] = (byte) (0x80 | (cp & 0x3F));
                } else {
                    room(4);
                    buf[count++] = (byte) (0xF0 | (cp >> 18));
                    buf[count++] = (byte) (0x80 | ((cp >> 12) & 0x3F));
                    buf[count++] = (byte) (0x80 | ((cp >> 6) & 0x3F));
                    buf[count++] = (byte) (0x80 | (cp & 0x3F));
                }
            }
            case 1, 2 -> {
                if (cp < (kind == 1 ? 0x80 : 0x100)) {
                    room(1);
                    buf[count++] = (byte) cp;
                } else {
                    replacement();
                }
            }
            default -> {
                if (cp >= 0x10000) {
                    room(4);
                    unit(Character.highSurrogate(cp));
                    unit(Character.lowSurrogate(cp));
                } else {
                    unit(cp);
                }
            }
        }
    }

    public void write(String str, int off, int len) throws IOException {
        if (len < 0) {
            throw new IndexOutOfBoundsException();
        }
        char[] cbuf = new char[len];
        str.getChars(off, off + len, cbuf, 0);
        write(cbuf, 0, len);
    }

    @Override
    public Writer append(CharSequence csq, int start, int end) throws IOException {
        CharSequence s = csq == null ? "null" : csq;
        return append(s.subSequence(start, end));
    }

    @Override
    public Writer append(CharSequence csq) throws IOException {
        String s = String.valueOf(csq);
        write(s, 0, s.length());
        return this;
    }

    public void flush() throws IOException {
        synchronized (lock) {
            ensureOpen();
            writeBytes();
            out.flush();
        }
    }

    public void close() throws IOException {
        synchronized (lock) {
            if (closed) {
                return;
            }
            try {
                try {
                    if (leftover >= 0) {
                        leftover = -1;
                        replacement();
                    }
                    writeBytes();
                    out.flush();
                } finally {
                    out.close();
                }
            } finally {
                closed = true;
            }
        }
    }
}
