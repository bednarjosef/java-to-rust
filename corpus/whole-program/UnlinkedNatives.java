import java.util.List;

// Natives whose code the program does not have, as a library's natives before its JNI library is
// loaded: Java finds no function for them and throws `UnsatisfiedLinkError` at each call, after
// evaluating the arguments, which a library that probes for its native code catches.
public class UnlinkedNatives {
    static class Inner {}

    static native int add(int a, int b);

    static native List<String> names(String prefix, int[] counts, Inner inner, long... rest);

    static native void touch();

    interface Probe {
        boolean present();
    }

    static final class NativeProbe implements Probe {
        public native boolean present();
    }

    static int calls;

    static int next() {
        return ++calls;
    }

    public static void main(String[] args) {
        try {
            System.out.println(add(next(), next()));
        } catch (UnsatisfiedLinkError e) {
            System.out.println(e.getMessage() + " after " + calls + " calls");
        }
        try {
            System.out.println(names("x", null, null).size());
        } catch (UnsatisfiedLinkError e) {
            System.out.println(e.getMessage());
        }
        try {
            touch();
            System.out.println("touched");
        } catch (LinkageError e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
        Probe probe = new NativeProbe();
        try {
            System.out.println(probe.present());
        } catch (Throwable t) {
            System.out.println("absent: " + t.getMessage());
        }
        try {
            System.loadLibrary("unlinkednatives");
            System.out.println("loaded");
        } catch (UnsatisfiedLinkError e) {
            System.out.println("no library");
        }
    }
}
