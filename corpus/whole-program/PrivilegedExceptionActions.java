// Actions that may throw, run with `AccessController.doPrivileged`: a checked exception comes out
// wrapped in a `PrivilegedActionException` (its cause, `getException` and `toString`), an
// unchecked one passes through as it is, and a value is returned; the same for
// `doPrivilegedWithCombiner`.
import java.io.IOException;
import java.security.AccessController;
import java.security.PrivilegedAction;
import java.security.PrivilegedActionException;
import java.security.PrivilegedExceptionAction;

public class PrivilegedExceptionActions {
    static int calls;

    static String read(String name) throws IOException {
        calls++;
        if (name.isEmpty()) {
            throw new IOException("no name");
        }
        return "read " + name;
    }

    @SuppressWarnings("removal")
    public static void main(String[] args) throws Exception {
        String value = AccessController.doPrivileged(new PrivilegedExceptionAction<String>() {
            @Override
            public String run() throws IOException {
                return read("a");
            }
        });
        System.out.println(value);

        try {
            AccessController.doPrivileged((PrivilegedExceptionAction<String>) () -> read(""));
            System.out.println("not thrown");
        } catch (PrivilegedActionException e) {
            System.out.println(e);
            System.out.println(e.getException() == e.getCause());
            System.out.println(e.getException().getMessage());
            System.out.println(e.getCause() instanceof IOException);
            System.out.println(e.getMessage());
        }

        try {
            AccessController.doPrivileged((PrivilegedExceptionAction<Integer>) () -> Integer.parseInt("x"));
        } catch (NumberFormatException e) {
            System.out.println("unchecked: " + e.getMessage());
        } catch (PrivilegedActionException e) {
            System.out.println("wrapped " + e);
        }

        try {
            AccessController.doPrivilegedWithCombiner((PrivilegedExceptionAction<String>) () -> {
                throw new Exception("plain");
            });
        } catch (PrivilegedActionException e) {
            System.out.println(e);
        }
        PrivilegedExceptionAction<String> b = () -> read("b");
        System.out.println(AccessController.doPrivilegedWithCombiner(b));
        System.out.println(AccessController.doPrivilegedWithCombiner((PrivilegedAction<Integer>) () -> 7));

        try {
            AccessController.doPrivileged((PrivilegedExceptionAction<Object>) () -> {
                throw new Error("an error");
            });
        } catch (Error e) {
            System.out.println("error: " + e.getMessage());
        }
        System.out.println(new PrivilegedActionException(null));
        System.out.println(calls);
    }
}
