// The arguments of a native the program does not have are evaluated before Java throws
// `UnsatisfiedLinkError`, and those that throw, such as a cast, an array index or a division,
// throw first: a narrowing cast to a peer class, as AWT's natives take, gives `ClassCastException`.
public class UnlinkedNativeArguments {
    interface Peer {
        int f();
    }

    static class XPeer implements Peer {
        public int f() {
            return 1;
        }

        long window() {
            return 2;
        }
    }

    static class YPeer implements Peer {
        public int f() {
            return 9;
        }
    }

    static native void focus(long window);

    static native boolean gray(XPeer peer);

    static native void sized(int size, Object o);

    static String run(Runnable r) {
        try {
            r.run();
            return "ran";
        } catch (RuntimeException | LinkageError e) {
            return e.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        Peer y = args.length > 7 ? null : new YPeer();
        Peer x = new XPeer();
        int[] sizes = {4};
        int zero = args.length;
        System.out.println(run(() -> focus(((XPeer) y).window())));
        System.out.println(run(() -> gray((XPeer) y)));
        System.out.println(run(() -> gray((XPeer) x)));
        System.out.println(run(() -> sized(sizes[1], x)));
        System.out.println(run(() -> sized(sizes[0] / zero, x)));
        System.out.println(run(() -> sized(sizes[0], (XPeer) y)));
        System.out.println(run(() -> sized(sizes[0], y)));
    }
}
