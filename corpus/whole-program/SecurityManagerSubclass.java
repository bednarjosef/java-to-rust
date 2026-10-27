// A class of the program may extend `SecurityManager`, as log4j's does: its objects are made, its
// own and the inherited methods run, and a variable of type `SecurityManager` that holds one is
// not null. There is still no security manager: `System.getSecurityManager()` is null. log4j and
// slf4j read the classes on the call stack through such a class and fall back where that fails,
// as the translation's `getClassContext` does: the caller found either way is the same.
@SuppressWarnings("removal")
public class SecurityManagerSubclass {
    static final class Stack extends SecurityManager {
        @Override
        protected Class<?>[] getClassContext() {
            return super.getClassContext();
        }
    }

    static Class<?> caller(Class<?> fallback) {
        try {
            Class<?>[] context = new Stack().getClassContext();
            return context.length > 1 ? context[1] : fallback;
        } catch (Exception e) {
            return fallback;
        }
    }

    static final class Counting extends SecurityManager {
        static int made;
        final String name;

        Counting(String name) {
            this.name = name;
            made++;
        }

        @Override
        public void checkExit(int status) {
            System.out.println(name + " allows exit " + status);
        }
    }

    public static void main(String[] args) {
        Counting counting = new Counting("first");
        SecurityManager sm = args.length > 3 ? null : counting;
        System.out.println(Counting.made + " " + (sm != null) + " " + (sm instanceof Counting));
        sm.checkExit(2);
        try {
            sm.checkPermission(new RuntimePermission("exitVM"));
        } catch (SecurityException e) {
            System.out.println("checkPermission: " + e);
        }
        try {
            sm.checkRead("file");
        } catch (SecurityException e) {
            System.out.println("checkRead: " + e);
        }
        SecurityManager none = System.getSecurityManager();
        System.out.println((none == null) + " " + (System.getSecurityManager() != null ? "managed" : "unmanaged"));
        System.out.println(new Counting("second").name + " " + Counting.made);
        System.out.println("caller: " + caller(SecurityManagerSubclass.class).getSimpleName());
    }
}
