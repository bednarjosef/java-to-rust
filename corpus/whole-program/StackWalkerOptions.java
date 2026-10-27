// A walker made with a set of options and a depth, as Minecraft's `TracyZoneFiller` makes one in
// a static field: Java checks both arguments.
import java.util.Set;

public class StackWalkerOptions {
    private static final StackWalker WALKER = StackWalker.getInstance(Set.of(StackWalker.Option.RETAIN_CLASS_REFERENCE), 5);

    public static void main(String[] args) {
        System.out.println(WALKER != null);
        try {
            StackWalker.getInstance(Set.of(), 0);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        try {
            StackWalker.getInstance((Set<StackWalker.Option>) null);
        } catch (NullPointerException e) {
            System.out.println("null options");
        }
    }
}
