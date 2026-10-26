// Locals that Java knows are assigned because an `if` tests a constant, as the JDK's `RSACore`
// assigns its blinding pair under `if (ENABLE_BLINDING)`.
public class ConstantConditionAssignments {
    private static final boolean ENABLED = true;
    private static final boolean DEBUG = false;

    static class Flags {
        static final boolean ON = !DEBUG && ENABLED;
    }

    static int twice(int n) {
        int doubled;
        if (ENABLED) {
            doubled = n * 2;
        }
        return doubled;
    }

    static String name(int n) {
        String s;
        if (Flags.ON) {
            s = "on " + n;
        } else {
            System.out.println("never");
        }
        return s;
    }

    static int unused(int n) {
        int never;
        if (DEBUG) {
            // Java lets a branch that never runs read a local that nothing assigned.
            System.out.println(never);
        }
        if ((DEBUG || !ENABLED)) {
            return -1;
        } else {
            never = n + 1;
        }
        return never;
    }

    public static void main(String[] args) {
        System.out.println(twice(21));
        System.out.println(name(3));
        System.out.println(unused(4));
    }
}
