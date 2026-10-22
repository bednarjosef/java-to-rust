// Actions run with `AccessController.doPrivileged`, which, with no security manager, runs them:
// an anonymous class, a lambda and a method reference, each returning its value.
import java.security.AccessController;
import java.security.PrivilegedAction;

public class PrivilegedActions {
    static String home() {
        return "home";
    }

    @SuppressWarnings("removal")
    public static void main(String[] args) {
        Boolean set = AccessController.doPrivileged(new PrivilegedAction<Boolean>() {
            @Override
            public Boolean run() {
                return "x".equals(System.getProperty("no.such.key", "x"));
            }
        });
        System.out.println(set);
        PrivilegedAction<Integer> sum = () -> 20 + 22;
        System.out.println(AccessController.doPrivileged(sum));
        PrivilegedAction<String> pa = PrivilegedActions::home;
        System.out.println(AccessController.doPrivileged(pa));
    }
}
