/**
 * A class's initialization initializes its superinterfaces that declare default methods, after
 * its superclass and in the order of its implements clauses, each interface's own superinterfaces
 * first; an interface without one is not initialized. A default method that the superclass's
 * constructor calls reads the interface's field set so, as Minecraft's blocks do.
 */
public class InterfaceInitialization {
    static String log = "";

    static String note(String s) {
        log += s + " ";
        return s.toLowerCase();
    }

    interface Plain {
        String P = note("Plain");
    }

    interface Grand {
        String G = note("Grand");

        default String g() {
            return G;
        }
    }

    interface WithDefault extends Plain, Grand {
        String D = note("WithDefault");

        default String amount() {
            return D;
        }
    }

    interface Other {
        String O = note("Other");

        default String o() {
            return O;
        }
    }

    abstract static class Base {
        static String B = note("Base");
        final String described;

        Base() {
            described = describe();
        }

        abstract String describe();
    }

    static class Impl extends Base implements WithDefault, Other {
        static String I = note("Impl");

        String describe() {
            return amount() + "," + o();
        }
    }

    /** An interface whose initializer only notes that it ran: no code reads its field. */
    interface Noisy {
        String N = note("Noisy");

        default int one() {
            return 1;
        }
    }

    static class Quiet implements Noisy {
        static String Q = note("Quiet");
    }

    public static void main(String[] args) {
        System.out.println(log.isEmpty());
        Impl impl = new Impl();
        System.out.println(log);
        System.out.println(impl.described + " " + impl.g());
        System.out.println(Plain.P + " " + log);
        System.out.println(new Quiet().one() + " " + log);
    }
}
