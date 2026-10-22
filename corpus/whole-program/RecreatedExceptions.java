import java.lang.reflect.Constructor;

/**
 * ForkJoinTask's way of making a new exception of another's class, with the other as its cause:
 * the public constructor of a Throwable, else the public one without parameters and initCause,
 * else the exception itself.
 */
public class RecreatedExceptions {
    public static class WithCause extends RuntimeException {
        public WithCause(String message) { super(message); }
        public WithCause(Throwable cause) { super(cause); }
    }

    public static class Bare extends RuntimeException {
        public Bare() { super("bare"); }
    }

    public static class Neither extends RuntimeException {
        public Neither(String message) { super(message); }
    }

    static class SamePackage extends RuntimeException {
        public SamePackage(Throwable cause) { super(cause); }
        SamePackage(String message) { super(message); }
    }

    public static class Hidden extends RuntimeException {
        Hidden(Throwable cause) { super(cause); }
        public Hidden(String message) { super(message); }
    }

    public class Inner extends RuntimeException {
        public Inner(Throwable cause) { super(cause); }
        public Inner(String message) { super(message); }
    }

    static Throwable recreate(Throwable ex) {
        try {
            Constructor<?> noArgCtor = null, oneArgCtor = null;
            for (Constructor<?> c : ex.getClass().getConstructors()) {
                Class<?>[] ps = c.getParameterTypes();
                if (ps.length == 0)
                    noArgCtor = c;
                else if (ps.length == 1 && ps[0] == Throwable.class) {
                    oneArgCtor = c;
                    break;
                }
            }
            if (oneArgCtor != null)
                ex = (Throwable)oneArgCtor.newInstance(ex);
            else if (noArgCtor != null) {
                Throwable rx = (Throwable)noArgCtor.newInstance();
                rx.initCause(ex);
                ex = rx;
            }
        } catch (Exception ignore) {
        }
        return ex;
    }

    static void show(String name, Throwable original) {
        Throwable made = recreate(original);
        Throwable cause = made == null ? null : made.getCause();
        System.out.println(name + ": " + (made == null ? "null" : made.getClass().getName())
            + " same=" + (made == original)
            + (made == null ? "" : " message=" + made.getMessage())
            + " cause=" + (cause == null ? "none" : cause.getClass().getName() + " " + (cause == original)));
    }

    public static void main(String[] args) {
        show("with cause", new WithCause("w"));
        show("bare", new Bare());
        show("neither", new Neither("n"));
        show("same package", new SamePackage("p"));
        show("hidden", new Hidden("h"));
        RecreatedExceptions outer = new RecreatedExceptions();
        show("inner", outer.new Inner("i"));
        show("standard", new IllegalStateException("s"));
        show("null", null);
    }
}
