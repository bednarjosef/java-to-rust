// A class of the program may extend `SecurityManager`, as log4j's does: its objects are made, its
// own and the inherited methods run, and a variable of type `SecurityManager` that holds one is
// not null. There is still no security manager: `System.getSecurityManager()` is null, and
// setting one throws.
@SuppressWarnings("removal")
public class SecurityManagerSubclass {
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
        try {
            System.setSecurityManager(sm);
        } catch (UnsupportedOperationException e) {
            System.out.println("setSecurityManager: " + e.getMessage());
        }
        System.out.println(new Counting("second").name + " " + Counting.made);
    }
}
