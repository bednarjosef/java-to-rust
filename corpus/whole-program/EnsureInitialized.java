import java.lang.invoke.MethodHandles;

// MethodHandles.lookup().ensureInitialized(c) runs the static initializer of c's class once, as
// the JDK's SharedSecrets does through a helper that passes its parameter on.
public class EnsureInitialized {
    static class Holder {
        static String value;
    }

    static class Registry {
        static {
            System.out.println("Registry initialized");
            Holder.value = "registered";
        }
    }

    static class Other {
        static int count = 0;

        static {
            System.out.println("Other initialized");
        }
    }

    static class Never {
        static {
            System.out.println("Never initialized");
        }
    }

    private static void ensure(Class<?> c) {
        try {
            MethodHandles.lookup().ensureInitialized(c);
        } catch (IllegalAccessException e) {
            System.out.println("no access");
        }
    }

    public static void main(String[] args) throws Exception {
        System.out.println("start " + Holder.value);
        ensure(Registry.class);
        System.out.println(Holder.value);
        ensure(Registry.class);
        Class<?> other = MethodHandles.lookup().ensureInitialized(Other.class);
        System.out.println(other.getSimpleName() + " " + Other.count);
        System.out.println(Never.class.getSimpleName());
    }
}
