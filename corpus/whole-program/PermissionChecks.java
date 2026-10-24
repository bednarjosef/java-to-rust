// `AccessController.checkPermission`, which without a security manager fails for every permission,
// as a library tests whether it may read class loaders only when a security manager is there.
import java.security.AccessControlException;
import java.security.AccessController;
import java.security.Permission;

public class PermissionChecks {
    static final RuntimePermission GET_CLASS_LOADER = new RuntimePermission("getClassLoader");

    @SuppressWarnings("removal")
    static boolean disabled() {
        if (System.getSecurityManager() == null) {
            return false;
        }
        try {
            AccessController.checkPermission(GET_CLASS_LOADER);
            return false;
        } catch (SecurityException e) {
            return true;
        }
    }

    @SuppressWarnings("removal")
    static String check(Permission permission) {
        try {
            AccessController.checkPermission(permission);
            return "allowed";
        } catch (AccessControlException e) {
            return e.getClass().getName() + ": " + e.getMessage() + " " + e.getPermission();
        }
    }

    @SuppressWarnings("removal")
    public static void main(String[] args) {
        System.out.println(disabled());
        System.out.println(GET_CLASS_LOADER.getName() + " " + GET_CLASS_LOADER.getActions().isEmpty());
        System.out.println(check(GET_CLASS_LOADER));
        System.out.println(check(null));
        try {
            AccessController.checkPermission(new RuntimePermission("exitVM"));
        } catch (SecurityException e) {
            System.out.println("security: " + e.getMessage());
        }
    }
}
