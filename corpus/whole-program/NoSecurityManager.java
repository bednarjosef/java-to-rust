// A plain run has no security manager: `System.getSecurityManager()` is null, held in a variable
// or tested where it is asked, and the checks that a manager would make are skipped.
public class NoSecurityManager {
    @SuppressWarnings("removal")
    public static void main(String[] args) {
        SecurityManager sm = System.getSecurityManager();
        if (sm != null) {
            sm.checkPermission(new RuntimePermission("exitVM"));
            System.out.println("checked");
        }
        System.out.println(sm == null);
        System.out.println(System.getSecurityManager() != null ? "managed" : "unmanaged");
    }
}
