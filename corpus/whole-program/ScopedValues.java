// Scoped values: bound and unbound reads, nested and rebinding carriers, `call` with a result,
// a checked exception through `call` and an unchecked one through `run` (the bindings are undone
// after both), and a thread started inside a binding, which does not inherit it.
import java.io.IOException;

public class ScopedValues {
    static final ScopedValue<String> USER = ScopedValue.newInstance();
    static final ScopedValue<Integer> DEPTH = ScopedValue.newInstance();

    static void show() {
        System.out.println("bound " + USER.isBound() + " " + USER.orElse("nobody") + " depth " + DEPTH.orElse(-1));
    }

    static String risky() throws IOException {
        if (USER.get().equals("ada")) {
            throw new IOException("no " + USER.get());
        }
        return "fine";
    }

    public static void main(String[] args) throws Exception {
        show();
        ScopedValue.where(USER, "ada").run(() -> {
            show();
            System.out.println("get " + USER.get());
            ScopedValue.where(USER, "bob").where(DEPTH, 2).run(ScopedValues::show);
            show();
        });
        show();
        String r = ScopedValue.where(DEPTH, 7).call(() -> "depth was " + DEPTH.get());
        System.out.println(r);
        try {
            USER.get();
        } catch (java.util.NoSuchElementException e) {
            System.out.println("unbound: " + e.getMessage());
        }
        try {
            ScopedValue.where(USER, "ada").call(ScopedValues::risky);
        } catch (IOException e) {
            System.out.println("caught " + e.getMessage() + " bound " + USER.isBound());
        }
        System.out.println(ScopedValue.where(USER, "bob").call(ScopedValues::risky));
        try {
            ScopedValue.where(USER, "eve").run(() -> {
                throw new IllegalStateException(USER.get());
            });
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage() + " " + USER.orElse("none"));
        }
        Thread t = new Thread(ScopedValues::show);
        ScopedValue.where(USER, "carol").run(() -> {
            t.start();
            try {
                t.join();
            } catch (InterruptedException e) {
            }
            show();
        });
    }
}
