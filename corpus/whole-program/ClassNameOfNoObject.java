// The class name of a value of an interface that no object of the program implements, which
// only null can be: its match over the classes has no arms but the one that never returns.
public class ClassNameOfNoObject {
    interface Listener {
        void done();
    }

    static final class Printing implements Listener {
        public void done() {
            System.out.println("done");
        }
    }

    static final class Logging implements Listener {
        public void done() {
            System.out.println("logged");
        }
    }

    static Listener listener;

    static String describe(Listener l) {
        return "listener " + l.getClass().getName();
    }

    public static void main(String[] args) {
        if (listener != null) {
            System.out.println(describe(listener));
        }
        System.out.println("none");
    }
}
