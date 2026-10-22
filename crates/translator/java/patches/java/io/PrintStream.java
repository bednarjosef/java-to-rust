package java.io;

/**
 * The translator's patch of the JDK's {@code PrintStream} (see docs/jdk-overlays.md, and
 * docs/natives-and-input.md, "System.out, System.err and System.in as values").
 *
 * <p>The JDK's class encodes its text through a {@code BufferedWriter} over an
 * {@code OutputStreamWriter}, whose {@code StreamEncoder} the translation cannot take yet.
 * Patched, it encodes each piece of text with {@code String.getBytes}, which the runtime does, and writes
 * the bytes through {@link #write(byte[], int, int)}, as the JDK's encoder does: the same bytes
 * reach the stream underneath, in the same calls of {@code write} and {@code flush} up to how
 * long text is cut into pieces. Errors set {@link #checkError()} and are not thrown, as in the
 * JDK. {@code printf} and {@code format} are formatted where they are called (the translator
 * replaces those calls with {@link #formattedText}), as {@code System.out.printf} is.
 */
class PrintStream {
    @Replace
    public PrintStream(OutputStream out, boolean autoFlush) {
        super(nonNull(out));
        this.autoFlush = autoFlush;
        this.charset = Charset.defaultCharset();
    }

    @Replace
    public PrintStream(OutputStream out, boolean autoFlush, String encoding) throws UnsupportedEncodingException {
        this(nonNull(out), autoFlush, charsetNamed(encoding));
    }

    @Replace
    public PrintStream(OutputStream out, boolean autoFlush, Charset charset) {
        super(out);
        this.autoFlush = autoFlush;
        this.charset = charset;
    }

    @Replace
    public PrintStream(String fileName) throws FileNotFoundException {
        this(new FileOutputStream(fileName), false);
    }

    @Replace
    public PrintStream(String fileName, String csn) throws FileNotFoundException, UnsupportedEncodingException {
        this(charsetNamed(csn), fileName);
    }

    @Replace
    public PrintStream(String fileName, Charset charset) throws IOException {
        this(new FileOutputStream(fileName), false, charset);
    }

    @Replace
    public PrintStream(File file) throws FileNotFoundException {
        this(new FileOutputStream(file), false);
    }

    @Replace
    public PrintStream(File file, String csn) throws FileNotFoundException, UnsupportedEncodingException {
        this(charsetNamed(csn), file);
    }

    @Replace
    public PrintStream(File file, Charset charset) throws IOException {
        this(new FileOutputStream(file), false, charset);
    }

    /** The charset is checked before the file is opened. */
    @Add
    private PrintStream(Charset charset, String fileName) throws FileNotFoundException {
        this(new FileOutputStream(fileName), false, charset);
    }

    @Add
    private PrintStream(Charset charset, File file) throws FileNotFoundException {
        this(new FileOutputStream(file), false, charset);
    }

    @Add
    private static OutputStream nonNull(OutputStream out) {
        if (out == null) {
            throw new NullPointerException("Null output stream");
        }
        return out;
    }

    @Add
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
    @Add
    private static IOException closed() {
        return new IOException("Stream closed");
    }

    @Replace
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

    @Replace
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

    @Replace
    public boolean checkError() {
        if (out != null) {
            flush();
        }
        if (out instanceof PrintStream inner) {
            return inner.checkError();
        }
        return trouble;
    }

    @Replace
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

    @Replace
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

    /**
     * Prints {@code text}, and with {@code newline} a line separator after it: its bytes go
     * through {@link #write(byte[], int, int)}, and with automatic flushing the stream is flushed
     * after a line, as the JDK's {@code PrintStream} does.
     */
    @Add
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

    @Replace
    public void print(boolean b) {
        text(String.valueOf(b), false);
    }

    @Replace
    public void print(char c) {
        text(String.valueOf(c), false);
    }

    @Replace
    public void print(int i) {
        text(String.valueOf(i), false);
    }

    @Replace
    public void print(long l) {
        text(String.valueOf(l), false);
    }

    @Replace
    public void print(float f) {
        text(String.valueOf(f), false);
    }

    @Replace
    public void print(double d) {
        text(String.valueOf(d), false);
    }

    @Replace
    public void print(char[] s) {
        text(new String(s), false);
    }

    @Replace
    public void print(String s) {
        text(String.valueOf(s), false);
    }

    @Replace
    public void print(Object obj) {
        text(String.valueOf(obj), false);
    }

    /** The line separator, as {@code println()} prints it. */
    @Replace
    private void newLine() {
        text("", true);
    }

    /**
     * Whether this is a {@code PrintStream} itself: {@code println} of a subclass prints through
     * its {@code print} and then the line separator, as the JDK's does, so that a subclass that
     * overrides {@code print} sees the text of {@code println} too.
     */
    @Add
    private boolean exact() {
        return getClass() == PrintStream.class;
    }

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
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

    @Replace
    public PrintStream format(String format, Object... args) {
        return formattedText(String.format(format, args));
    }

    @Replace
    public PrintStream format(Locale l, String format, Object... args) {
        return formattedText(String.format(l, format, args));
    }

    /** What {@code printf} and {@code format} print, formatted where they are called. */
    @Add
    PrintStream formattedText(String text) {
        print(text);
        return this;
    }

    @Replace
    public PrintStream append(CharSequence csq, int start, int end) {
        // The translation has no `CharSequence.subSequence`; for a string or a builder, the
        // substring of its text is the same, with the same exception.
        String s = String.valueOf(csq);
        return append(s.substring(start, end));
    }

    @Remove
    private Formatter formatter;

    @Remove
    private BufferedWriter textOut;

    @Remove
    private OutputStreamWriter charOut;

    @Remove
    private static <T> T requireNonNull(T obj, String message);

    @Remove
    private static Charset toCharset(String csn) throws UnsupportedEncodingException;

    @Remove
    private PrintStream(boolean autoFlush, OutputStream out);

    @Remove
    private PrintStream(boolean autoFlush, Charset charset, OutputStream out);

    @Remove
    private void ensureOpen() throws IOException;

    @Remove
    private void write(char[] buf);

    @Remove
    private void writeln(char[] buf);

    @Remove
    private void write(String s);

    @Remove
    private void writeln(String s);
}
