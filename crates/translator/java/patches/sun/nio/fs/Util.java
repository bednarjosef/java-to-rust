package sun.nio.fs;

/**
 * The translator's patch of the JDK's {@code sun.nio.fs.Util} (see docs/jdk-overlays.md). The
 * JDK's keeps the encoding of file names in a static field, which it reads from
 * {@code StaticProperty}; a static initializer, which the translation lets fail with an
 * {@code Error} (see {@code lower/clinit_errors.rs}), makes every message of a
 * {@code UnixException} look as if it could throw. Patched, the class has none: it finds the
 * charset of the same property, which {@code System.getProperty} answers as the JVM does, each
 * time.
 */
class Util {
    @Remove
    private static final Charset jnuEncoding;

    @Edit(find = "return jnuEncoding;", replace = "return Charset.forName(System.getProperty(\"sun.jnu.encoding\"));")
    static Charset jnuEncoding();

    @Edit(find = "s.getBytes(jnuEncoding)", replace = "s.getBytes(jnuEncoding())")
    static byte[] toBytes(String s);

    // No caller passes null, but the translation's nullness is not flow-sensitive: the null that
    // `readdir` gives at the end of a directory, which `UnixDirectoryStream` tests before it makes
    // a path of the name, makes every path's bytes look as if they could be null, and every
    // message of a `UnixException` as if it could throw. A null gives null here, where the JDK's
    // would throw.
    @Edit(find = "return new String(bytes, jnuEncoding);", replace = """
            if (bytes == null) {
                return null;
            }
            return new String(bytes, jnuEncoding());
            """)
    static String toString(byte[] bytes);
}
