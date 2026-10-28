package java.lang;

import java.io.File;
import java.io.IOException;
import jdk.internal.reflect.Reflection;

/**
 * The translator's {@code System.loadLibrary}, {@code System.load} and
 * {@code System.mapLibraryName}. A translated program has no JNI: the natives it can call are
 * the runtime's, a call of a library's native throws {@code UnsatisfiedLinkError} as where its
 * library was never loaded, and a call of any other native is rejected where the translator meets
 * it (see {@code lower/natives.rs}). Loading a library therefore adds nothing that the program could
 * call, and only its outcome is left: it succeeds where Java finds the library's file and throws
 * Java's {@code UnsatisfiedLinkError} where Java finds none, searching the same directories in
 * the same order ({@code sun.boot.library.path}, then {@code java.library.path} for a class that
 * the boot loader did not define). A file that is there but that the system cannot load (not a
 * library, or one whose dependencies are missing) loads in the translation, where Java throws.
 *
 * <p>Code in a module that Java does not give native access (any but the JDK's modules that
 * {@code lower.rs} names) calls {@code loadLibraryUnlinked} and {@code loadUnlinked} instead. Java
 * loads its library after a warning, and the library's natives are then that code's own, which a
 * translation does not have: these throw {@code UnsatisfiedLinkError} as Java does when it cannot
 * load the library, with Java's message where Java finds no file, and one that says so where it
 * finds one. Java's warning, which names where the caller's class was loaded from, is left out:
 * the translation does not know that place, and loads nothing.
 */
final class TranslatedLibraries {
    private TranslatedLibraries() {}

    /** {@code System.loadLibrary(libname)}, as {@code Runtime.loadLibrary0} does it. */
    static void loadLibrary(String libname) {
        Class<?> caller = Reflection.getCallerClass();
        search(libname, caller.getClassLoader() == null);
    }

    /** {@code System.loadLibrary(libname)} of code without native access. */
    static void loadLibraryUnlinked(String libname) {
        throw unlinked(search(libname, false));
    }

    /** {@code System.load(filename)}, as {@code Runtime.load0} does it. */
    static void load(String filename) {
        check(filename);
    }

    /** {@code System.load(filename)} of code without native access. */
    static void loadUnlinked(String filename) {
        throw unlinked(check(filename));
    }

    /**
     * The file of library {@code libname} in the directories that Java searches: those of
     * {@code sun.boot.library.path}, then, unless the caller's class is the boot loader's
     * ({@code boot}), those of {@code java.library.path}. Where none has it, Java's
     * {@code UnsatisfiedLinkError}.
     */
    private static File search(String libname, boolean boot) {
        if (libname.indexOf((int) File.separatorChar) != -1) {
            throw new UnsatisfiedLinkError(
                "Directory separator should not appear in library name: " + libname);
        }
        String bootPath = System.getProperty("sun.boot.library.path");
        File file = found(bootPath, libname);
        if (file != null) {
            return file;
        }
        if (boot) {
            throw new UnsatisfiedLinkError("no " + libname + " in system library path: " + bootPath);
        }
        String userPath = System.getProperty("java.library.path");
        file = found(userPath, libname);
        if (file == null) {
            throw new UnsatisfiedLinkError("no " + libname + " in java.library.path: " + userPath);
        }
        return file;
    }

    /** The library file {@code filename}, if Java would load it, else Java's error. */
    private static File check(String filename) {
        File file = new File(filename);
        if (!file.isAbsolute()) {
            throw new UnsatisfiedLinkError("Expecting an absolute path of the library: " + filename);
        }
        if (!loads(file)) {
            throw new UnsatisfiedLinkError("Can't load library: " + file);
        }
        return file;
    }

    /** The error for the library {@code file}, which Java would load and a translation cannot. */
    private static UnsatisfiedLinkError unlinked(File file) {
        return new UnsatisfiedLinkError(
            "Can't load library: " + file + " (a translated program has no native code)");
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
     * The file of library {@code name} in the first directory of the search path {@code path}
     * that has it, as {@code NativeLibraries.findFromPaths} looks (an empty element of the path
     * is the current directory), or null.
     */
    private static File found(String path, String name) {
        String file = mapLibraryName(name);
        int start = 0;
        while (true) {
            int end = path.indexOf(File.pathSeparatorChar, start);
            String dir = end < 0 ? path.substring(start) : path.substring(start, end);
            File candidate = new File(dir.isEmpty() ? "." : dir, file);
            if (loads(candidate)) {
                return candidate;
            }
            if (end < 0) {
                return null;
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
