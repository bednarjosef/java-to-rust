package java.io;

import java.nio.charset.Charset;
import java.util.Locale;

/**
 * The translator's {@code PrintStream}, which replaces the JDK's in a translation with the JDK's
 * sources (see docs/natives-and-input.md, "System.out, System.err and System.in as values").
 *
 * <p>The JDK's class encodes its text through a {@code BufferedWriter} over an
 * {@code OutputStreamWriter}, whose {@code StreamEncoder} the translation cannot take yet. This
 * one encodes each piece of text with {@code String.getBytes}, which the runtime does, and writes
 * the bytes through {@link #write(byte[], int, int)}, as the JDK's encoder does: the same bytes
 * reach the stream underneath, in the same calls of {@code write} and {@code flush} up to how
 * long text is cut into pieces. Errors set {@link #checkError()} and are not thrown, as in the
 * JDK. {@code printf} and {@code format} are formatted where they are called (the translator
 * replaces those calls with {@link #formattedText}), as {@code System.out.printf} is.
 */
public class PrintStream extends FilterOutputStream implements Appendable, Closeable {
    private final boolean autoFlush;
    private final Charset charset;
    private boolean trouble = false;
    private boolean closing = false;

    public PrintStream(OutputStream out) {
        this(out, false);
    }

    public PrintStream(OutputStream out, boolean autoFlush) {
        super(nonNull(out));
        this.autoFlush = autoFlush;
        this.charset = Charset.defaultCharset();
    }

    public PrintStream(OutputStream out, boolean autoFlush, String encoding) throws UnsupportedEncodingException {
        this(nonNull(out), autoFlush, charsetNamed(encoding));
    }

    public PrintStream(OutputStream out, boolean autoFlush, Charset charset) {
        super(out);
        this.autoFlush = autoFlush;
        this.charset = charset;
    }

    public PrintStream(String fileName) throws FileNotFoundException {
        this(new FileOutputStream(fileName), false);
    }

    public PrintStream(String fileName, String csn) throws FileNotFoundException, UnsupportedEncodingException {
        this(charsetNamed(csn), fileName);
    }

    public PrintStream(String fileName, Charset charset) throws IOException {
        this(new FileOutputStream(fileName), false, charset);
    }

    public PrintStream(File file) throws FileNotFoundException {
        this(new FileOutputStream(file), false);
    }

    public PrintStream(File file, String csn) throws FileNotFoundException, UnsupportedEncodingException {
        this(charsetNamed(csn), file);
    }

    public PrintStream(File file, Charset charset) throws IOException {
        this(new FileOutputStream(file), false, charset);
    }

    /** The charset is checked before the file is opened. */
    private PrintStream(Charset charset, String fileName) throws FileNotFoundException {
        this(new FileOutputStream(fileName), false, charset);
    }

    private PrintStream(Charset charset, File file) throws FileNotFoundException {
        this(new FileOutputStream(file), false, charset);
    }

    private static OutputStream nonNull(OutputStream out) {
        if (out == null) {
            throw new NullPointerException("Null output stream");
        }
        return out;
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

    /**
     * The {@code IOException} of a stream that is closed: the stream underneath is null. Each
     * method reads {@code out} into a local and tests it, so that the translation knows the
     * stream it writes to is not null.
     */
    private static IOException closed() {
        return new IOException("Stream closed");
    }

    @Override
    public void flush() {
        synchronized (this) {
            try {
                OutputStream stream = out;
                if (stream == null) {
                    throw closed();
                }
                stream.flush();
            } catch (IOException e) {
                trouble = true;
            }
        }
    }

    @Override
    public void close() {
        synchronized (this) {
            if (closing) {
                return;
            }
            closing = true;
            try {
                OutputStream stream = out;
                if (stream != null) {
                    stream.close();
                }
            } catch (IOException e) {
                trouble = true;
            }
            out = null;
        }
    }

    public boolean checkError() {
        if (out != null) {
            flush();
        }
        if (out instanceof PrintStream inner) {
            return inner.checkError();
        }
        return trouble;
    }

    protected void setError() {
        trouble = true;
    }

    protected void clearError() {
        trouble = false;
    }

    @Override
    public void write(int b) {
        synchronized (this) {
            try {
                OutputStream stream = out;
                if (stream == null) {
                    throw closed();
                }
                stream.write(b);
                if (b == '\n' && autoFlush) {
                    stream.flush();
                }
            } catch (IOException e) {
                trouble = true;
            }
        }
    }

    @Override
    public void write(byte[] buf, int off, int len) {
        synchronized (this) {
            try {
                OutputStream stream = out;
                if (stream == null) {
                    throw closed();
                }
                stream.write(buf, off, len);
                if (autoFlush) {
                    stream.flush();
                }
            } catch (IOException e) {
                trouble = true;
            }
        }
    }

    @Override
    public void write(byte[] buf) throws IOException {
        this.write(buf, 0, buf.length);
    }

    public void writeBytes(byte[] buf) {
        this.write(buf, 0, buf.length);
    }

    /**
     * Prints {@code text}, and with {@code newline} a line separator after it: its bytes go
     * through {@link #write(byte[], int, int)}, and with automatic flushing the stream is flushed
     * after a line, as the JDK's {@code PrintStream} does.
     */
    private void text(String text, boolean newline) {
        synchronized (this) {
            try {
                if (out == null) {
                    throw closed();
                }
                String whole = newline ? text + System.lineSeparator() : text;
                byte[] bytes = whole.getBytes(charset);
                if (bytes.length > 0) {
                    write(bytes, 0, bytes.length);
                }
                if (autoFlush && (newline || whole.indexOf('\n') >= 0)) {
                    OutputStream stream = out;
                    if (stream == null) {
                        throw closed();
                    }
                    stream.flush();
                }
            } catch (IOException e) {
                trouble = true;
            }
        }
    }

    public void print(boolean b) {
        text(String.valueOf(b), false);
    }

    public void print(char c) {
        text(String.valueOf(c), false);
    }

    public void print(int i) {
        text(String.valueOf(i), false);
    }

    public void print(long l) {
        text(String.valueOf(l), false);
    }

    public void print(float f) {
        text(String.valueOf(f), false);
    }

    public void print(double d) {
        text(String.valueOf(d), false);
    }

    public void print(char[] s) {
        text(new String(s), false);
    }

    public void print(String s) {
        text(String.valueOf(s), false);
    }

    public void print(Object obj) {
        text(String.valueOf(obj), false);
    }

    public void println() {
        newLine();
    }

    /** The line separator, as {@code println()} prints it. */
    private void newLine() {
        text("", true);
    }

    /**
     * Whether this is a {@code PrintStream} itself: {@code println} of a subclass prints through
     * its {@code print} and then the line separator, as the JDK's does, so that a subclass that
     * overrides {@code print} sees the text of {@code println} too.
     */
    private boolean exact() {
        return getClass() == PrintStream.class;
    }

    public void println(boolean x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(char x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(int x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(long x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(float x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(double x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(char[] x) {
        if (exact()) {
            text(new String(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(String x) {
        if (exact()) {
            text(String.valueOf(x), true);
        } else {
            synchronized (this) {
                print(x);
                newLine();
            }
        }
    }

    public void println(Object x) {
        // The object becomes text before anything is printed, as in the JDK.
        String s = String.valueOf(x);
        if (exact()) {
            text(s, true);
        } else {
            synchronized (this) {
                print(s);
                newLine();
            }
        }
    }

    public PrintStream printf(String format, Object... args) {
        return format(format, args);
    }

    public PrintStream printf(Locale l, String format, Object... args) {
        return format(l, format, args);
    }

    public PrintStream format(String format, Object... args) {
        return formattedText(String.format(format, args));
    }

    public PrintStream format(Locale l, String format, Object... args) {
        return formattedText(String.format(l, format, args));
    }

    /** What {@code printf} and {@code format} print, formatted where they are called. */
    PrintStream formattedText(String text) {
        print(text);
        return this;
    }

    public PrintStream append(CharSequence csq) {
        print(String.valueOf(csq));
        return this;
    }

    public PrintStream append(CharSequence csq, int start, int end) {
        // The translation has no `CharSequence.subSequence`; for a string or a builder, the
        // substring of its text is the same, with the same exception.
        String s = String.valueOf(csq);
        return append(s.substring(start, end));
    }

    public PrintStream append(char c) {
        print(c);
        return this;
    }

    public Charset charset() {
        return charset;
    }
}
