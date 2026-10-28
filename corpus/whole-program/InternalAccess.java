import jdk.internal.misc.Unsafe;

/**
 * A class compiled against the JDK's internals and run without their export, as joml's is: its
 * uses of them throw IllegalAccessError, which the code catches to fall back.
 */
public class InternalAccess {
    static final class Internal {
        static final Unsafe UNSAFE = Unsafe.getUnsafe();

        static long pageSize() {
            return UNSAFE.pageSize();
        }
    }

    static String message(Throwable e) {
        return e.getClass().getName() + ": " + e.getMessage().replaceAll(" @0x[0-9a-f]+", "");
    }

    public static void main(String[] args) {
        try {
            System.out.println(Internal.pageSize() > 0);
        } catch (Throwable e) {
            System.out.println("falls back: " + message(e));
        }
        try {
            System.out.println(Unsafe.ADDRESS_SIZE);
        } catch (IllegalAccessError e) {
            System.out.println("field: " + message(e));
        }
        try {
            System.out.println(new jdk.internal.util.NullableKeyValueHolder<>("key", 1).getKey());
        } catch (IllegalAccessError e) {
            System.out.println("new: " + message(e));
        }
    }
}
