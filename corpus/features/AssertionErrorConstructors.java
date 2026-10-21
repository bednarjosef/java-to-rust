// Every constructor of `AssertionError`: the text of an object (a `Throwable` also becomes the
// cause), each primitive shown as Java shows it, a `null` object, and `(String, Throwable)`.
public class AssertionErrorConstructors {
    static void show(AssertionError e) {
        System.out.println(e);
        System.out.println("message: " + e.getMessage());
        System.out.println("cause: " + e.getCause());
    }

    public static void main(String[] args) {
        show(new AssertionError());
        show(new AssertionError("text"));
        show(new AssertionError((Object) "object"));
        show(new AssertionError((Object) null));
        show(new AssertionError(Integer.valueOf(5)));
        show(new AssertionError(new int[] {1, 2}.length));
        show(new AssertionError(true));
        show(new AssertionError('c'));
        show(new AssertionError((short) -7));
        show(new AssertionError(42L));
        show(new AssertionError(1.5f));
        show(new AssertionError(0.1f + 0.2f));
        show(new AssertionError(1e10));
        show(new AssertionError(1.0 / 3));
        show(new AssertionError(-0.0));

        IllegalStateException inner = new IllegalStateException("inner");
        show(new AssertionError(inner));
        System.out.println(new AssertionError(inner).getCause() == inner);
        show(new AssertionError("wrapped", inner));
        show(new AssertionError("no cause", null));
        // Typed as `Object`, but a `Throwable` at run time: it is still the cause.
        show(new AssertionError((Object) new RuntimeException("typed as Object")));
    }
}
