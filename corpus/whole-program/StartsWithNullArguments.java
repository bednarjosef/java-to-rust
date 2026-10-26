// A null prefix of `startsWith` or suffix of `endsWith`, which `String`'s code dereferences, caught
// and printed, as slf4j's `LoggerFactory.versionSanityCheck` catches whatever its check of the
// provider's version throws, where the elements of string arrays may be null.
public class StartsWithNullArguments {
    interface Provider {
        String version();
    }

    static final String[] COMPATIBLE = {"2.0"};
    static String[] holes = {"a", null};
    static Provider provider;

    static void versionCheck() {
        try {
            String requested = provider.version();
            boolean match = false;
            for (String compatible : COMPATIBLE) {
                if (requested.startsWith(compatible)) {
                    match = true;
                }
            }
            System.out.println("match " + match);
        } catch (Throwable e) {
            System.out.println("Unexpected problem occurred during version sanity check: " + e);
        }
    }

    static void ends(String s, String suffix) {
        try {
            System.out.println(s.endsWith(suffix));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        provider = () -> "2.0.99";
        versionCheck();
        provider = () -> args.length > 3 ? "x" : null;
        versionCheck();
        COMPATIBLE[0] = holes[args.length + 1];
        provider = () -> "2.0.99";
        versionCheck();
        ends("abc", "bc");
        ends("abc", holes[args.length + 1]);
        ends(holes[args.length + 1], "c");
        ends(holes[args.length + 1], holes[args.length + 1]);
    }
}
