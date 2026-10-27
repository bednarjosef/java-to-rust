import java.io.File;

// A library's code is native, which a translated program does not have; a library that Java
// does not find either fails the same way in both: `UnsatisfiedLinkError`, which libraries such
// as netty, JNA and lz4 catch to fall back to code of their own.
public class MissingNativeLibraries {
    static void attempt(String what, Runnable load) {
        try {
            load.run();
            System.out.println(what + ": loaded");
        } catch (UnsatisfiedLinkError e) {
            System.out.println(what + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        System.out.println(System.mapLibraryName("no-such-library"));
        attempt("loadLibrary", () -> System.loadLibrary("no-such-library-for-the-corpus"));
        attempt("loadLibrary again", () -> System.loadLibrary("no-such-library-for-the-corpus"));
        attempt("loadLibrary with a separator", () -> System.loadLibrary("dir" + File.separator + "lib"));
        attempt("load", () -> System.load("/no/such/directory/libmissing.so"));
        attempt("load of a relative path", () -> System.load("libmissing.so"));
        attempt("Runtime.loadLibrary", () -> Runtime.getRuntime().loadLibrary("no-such-library-for-the-corpus"));
        boolean fellBack;
        try {
            System.loadLibrary("no-such-library-for-the-corpus");
            fellBack = false;
        } catch (Throwable t) {
            fellBack = t instanceof LinkageError;
        }
        System.out.println("fell back: " + fellBack);
    }
}
