package sun.nio.fs;

import java.nio.charset.Charset;
import java.nio.file.LinkOption;
import java.util.HashSet;
import java.util.Set;

/**
 * The translator's {@code sun.nio.fs.Util}, which replaces the JDK's in a translation with the
 * JDK's sources. The JDK's keeps the encoding of file names in a static field, which it reads
 * from {@code StaticProperty}; a static initializer, which the translation lets fail with an
 * {@code Error} (see {@code lower/clinit_errors.rs}), makes every message of a
 * {@code UnixException} look as if it could throw. This class has none: it finds the charset of
 * the same property, which {@code System.getProperty} answers as the JVM does, each time. The
 * rest is the JDK's.
 */
class Util {
    private Util() { }

    /** The charset that file names are encoded in. */
    static Charset jnuEncoding() {
        return Charset.forName(System.getProperty("sun.jnu.encoding"));
    }

    /** {@code s} as the bytes of a file name. */
    static byte[] toBytes(String s) {
        return s.getBytes(jnuEncoding());
    }

    /**
     * The bytes of a file name as text. No caller passes null, but the translation's nullness is
     * not flow-sensitive: the null that {@code readdir} gives at the end of a directory, which
     * {@code UnixDirectoryStream} tests before it makes a path of the name, makes every path's
     * bytes look as if they could be null, and every message of a {@code UnixException} as if it
     * could throw. A null gives null here, where the JDK's would throw.
     */
    static String toString(byte[] bytes) {
        if (bytes == null) {
            return null;
        }
        return new String(bytes, jnuEncoding());
    }

    /** {@code s} split at each {@code c}, keeping empty parts. */
    static String[] split(String s, char c) {
        int count = 0;
        for (int i = 0; i < s.length(); i++) {
            if (s.charAt(i) == c) {
                count++;
            }
        }
        String[] result = new String[count + 1];
        int n = 0;
        int last = 0;
        for (int i = 0; i < s.length(); i++) {
            if (s.charAt(i) == c) {
                result[n++] = s.substring(last, i);
                last = i + 1;
            }
        }
        result[n] = s.substring(last, s.length());
        return result;
    }

    @SafeVarargs
    static <E> Set<E> newSet(E... elements) {
        HashSet<E> set = new HashSet<>();
        for (E e : elements) {
            set.add(e);
        }
        return set;
    }

    @SafeVarargs
    static <E> Set<E> newSet(Set<E> other, E... elements) {
        HashSet<E> set = new HashSet<>(other);
        for (E e : elements) {
            set.add(e);
        }
        return set;
    }

    /** Whether symbolic links are followed: unless {@code NOFOLLOW_LINKS} is among the options. */
    static boolean followLinks(LinkOption... options) {
        boolean followLinks = true;
        for (LinkOption option : options) {
            if (option == LinkOption.NOFOLLOW_LINKS) {
                followLinks = false;
            } else if (option == null) {
                throw new NullPointerException();
            } else {
                throw new AssertionError("Should not get here");
            }
        }
        return followLinks;
    }
}
