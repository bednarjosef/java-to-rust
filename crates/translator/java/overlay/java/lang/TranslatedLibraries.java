package java.lang;

import java.io.File;
import java.io.IOException;
import jdk.internal.reflect.Reflection;

/**
 * The translator's {@code System.loadLibrary}, {@code System.load} and
 * {@code System.mapLibraryName}. A translated program has no JNI: the natives it can call are
 * the runtime's, and a call of any other native is rejected where the translator meets it (see
 * {@code lower/natives.rs}). Loading a library therefore adds nothing that the program could
 * call, and only its outcome is left: it succeeds where Java finds the library's file and throws
 * Java's {@code UnsatisfiedLinkError} where Java finds none, searching the same directories in
 * the same order ({@code sun.boot.library.path}, then {@code java.library.path} for a class that
 * the boot loader did not define). A file that is there but that the system cannot load (not a
 * library, or one whose dependencies are missing) loads in the translation, where Java throws.
 *
 * <p>The translator calls these only from the JDK's modules that Java gives native access
 * ({@code lower.rs}): from any other, Java prints a warning naming where the caller's class was
 * loaded from, which a translation does not know, and the call is rejected.
 */
final class TranslatedLibraries {
    private TranslatedLibraries() {}

    /** {@code System.loadLibrary(libname)}, as {@code Runtime.loadLibrary0} does it. */
    static void loadLibrary(String libname) {
        Class<?> caller = Reflection.getCallerClass();
        if (libname.indexOf((int) File.separatorChar) != -1) {
            throw new UnsatisfiedLinkError(
                "Directory separator should not appear in library name: " + libname);
        }
        String boot = System.getProperty("sun.boot.library.path");
        if (found(boot, libname)) {
            return;
        }
        if (caller.getClassLoader() == null) {
            throw new UnsatisfiedLinkError("no " + libname + " in system library path: " + boot);
        }
        String user = System.getProperty("java.library.path");
        if (found(user, libname)) {
            return;
        }
        throw new UnsatisfiedLinkError("no " + libname + " in java.library.path: " + user);
    }

    /** {@code System.load(filename)}, as {@code Runtime.load0} does it. */
    static void load(String filename) {
        File file = new File(filename);
        if (!file.isAbsolute()) {
            throw new UnsatisfiedLinkError("Expecting an absolute path of the library: " + filename);
        }
        if (!loads(file)) {
            throw new UnsatisfiedLinkError("Can't load library: " + file);
        }
    }

    /**
     * {@code System.mapLibraryName(libname)}, as the JDK's native does it: the name of the
     * library's file on this system.
     */
    static String mapLibraryName(String libname) {
        if (libname == null) {
            throw new NullPointerException();
        }
        if (libname.length() > 240) {
            throw new IllegalArgumentException("name too long");
        }
        String os = System.getProperty("os.name");
        if (os.startsWith("Windows")) {
            return libname + ".dll";
        }
        return "lib" + libname + (os.startsWith("Mac") ? ".dylib" : ".so");
    }

    /**
     * Whether a directory of the search path {@code path} has the file of library {@code name},
     * as {@code NativeLibraries.findFromPaths} looks: an empty element of the path is the
     * current directory.
     */
    private static boolean found(String path, String name) {
        String file = mapLibraryName(name);
        int start = 0;
        while (true) {
            int end = path.indexOf(File.pathSeparatorChar, start);
            String dir = end < 0 ? path.substring(start) : path.substring(start, end);
            if (loads(new File(dir.isEmpty() ? "." : dir, file))) {
                return true;
            }
            if (end < 0) {
                return false;
            }
            start = end + 1;
        }
    }

    /**
     * Whether Java would go on to load {@code file}: it is there, and has a canonical path
     * ({@code NativeLibraries.loadLibrary}).
     */
    private static boolean loads(File file) {
        if (!file.exists()) {
            return false;
        }
        try {
            file.getCanonicalPath();
            return true;
        } catch (IOException e) {
            return false;
        }
    }
}
