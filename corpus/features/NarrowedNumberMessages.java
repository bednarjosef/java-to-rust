// The messages of NullPointerExceptions of a null Number's byteValue() and shortValue(), which
// the translation computes from intValue(), and of Thread.holdsLock(null), whose exception has
// no message.
public class NarrowedNumberMessages {
    interface Source {
        Number get();
    }

    static byte byteOf(Object o) {
        return ((Number) o).byteValue();
    }

    static short shortOf(Source s) {
        return s.get().shortValue();
    }

    static int narrowed(Number n) {
        return (byte) n.intValue();
    }

    static class Lock {}

    static boolean held(Lock o) {
        return Thread.holdsLock(o);
    }

    interface Action {
        void run();
    }

    static void report(Action action) {
        try {
            action.run();
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        report(() -> System.out.println(byteOf(300)));
        report(() -> byteOf(null));
        report(() -> System.out.println(shortOf(() -> 70000L)));
        report(() -> shortOf(() -> null));
        report(() -> System.out.println(narrowed(1000)));
        report(() -> narrowed(null));
        report(() -> System.out.println(held(new Lock())));
        report(() -> held(null));
    }
}
