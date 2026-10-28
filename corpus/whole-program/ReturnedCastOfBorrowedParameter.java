import java.util.Optional;
import java.util.Set;
import java.util.function.Function;

// A string parameter the method only reads, possibly null, returned through a cast to its own type:
// `return (String) key;` hands back an owned copy of the borrowed string.
public class ReturnedCastOfBorrowedParameter {
    static final Set<String> SPECIAL = Set.of("dummy", "trigger");

    static String pick(String key) {
        if (key == null || SPECIAL.contains(key)) {
            return (String) key;
        }
        return "stat:" + key;
    }

    public static void main(String[] args) {
        System.out.println(pick("dummy"));
        System.out.println(pick("jump"));
        System.out.println(pick(null));
        Function<String, String> fix = key -> {
            if (SPECIAL.contains(key)) {
                return (String) key;
            }
            return key.length() > 3 ? "long:" + key : null;
        };
        System.out.println(Optional.of("trigger").map(fix).orElse("none"));
        System.out.println(Optional.of("walk").map(fix).orElse("none"));
        System.out.println(Optional.of("x").map(fix).orElse("none"));
    }
}
