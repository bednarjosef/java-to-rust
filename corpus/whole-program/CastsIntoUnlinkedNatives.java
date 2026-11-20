// Casts written as arguments of natives whose code the program cannot have: each cast is checked
// before the call throws `UnsatisfiedLinkError`, and its `ClassCastException` reaches the caller's
// `catch`, as the peer casts of the JDK's AWT classes do.
public class CastsIntoUnlinkedNatives {
    interface Peer {}

    static class Window implements Peer {
        long content() {
            return 7;
        }
    }

    static class Other implements Peer {}

    static native boolean opaque(Window w);

    native void focus(long window, boolean value);

    static boolean check(Peer p) {
        return opaque((Window) p);
    }

    void set(Peer p) {
        if (p == null) {
            return;
        }
        focus(((Window) p).content(), true);
    }

    static void kept(Peer p) {
        opaque((Window) p);
    }

    public static void main(String[] args) {
        for (Peer p : new Peer[] {new Other(), new Window(), null}) {
            try {
                System.out.println(check(p));
            } catch (ClassCastException e) {
                System.out.println("check: cast " + e.getClass().getName());
            } catch (UnsatisfiedLinkError e) {
                System.out.println("check: unlinked " + e.getMessage());
            }
            try {
                new CastsIntoUnlinkedNatives().set(p);
                System.out.println("set");
            } catch (ClassCastException e) {
                System.out.println("set: cast " + e.getClass().getName());
            } catch (UnsatisfiedLinkError e) {
                System.out.println("set: unlinked " + e.getMessage());
            }
            try {
                kept(p);
            } catch (RuntimeException e) {
                System.out.println("kept: " + e.getClass().getName());
            } catch (Error e) {
                System.out.println("kept: " + e.getClass().getName());
            }
        }
    }
}
