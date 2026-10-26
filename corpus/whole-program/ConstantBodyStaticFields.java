import java.util.ArrayList;
import java.util.List;

// Enum constants whose bodies declare static fields, as the Minecraft server's
// `SnbtGrammar.ArrayPrefix.BYTE { private static final ByteBuffer EMPTY_BUFFER = ..; }`. Their
// initializers have no effects, as they run after every constant (`javac::enum_bodies`).
public class ConstantBodyStaticFields {
    static final List<String> LOG = new ArrayList<>();

    static String made(String what) {
        LOG.add(what);
        return what;
    }

    enum Prefix {
        BYTE("b") {
            private static final String EMPTY = "empty".concat(" bytes");
            private static int created;

            @Override
            String create() {
                created++;
                return EMPTY + " " + created;
            }
        },
        INT("i") {
            private static final String NONE = "empty".concat(" ints");

            @Override
            String create() {
                return NONE + " of " + suffix;
            }
        },
        LONG("l");

        final String suffix;

        Prefix(String suffix) {
            this.suffix = made(suffix);
        }

        String create() {
            return "plain " + suffix;
        }
    }

    public static void main(String[] args) {
        for (Prefix p : Prefix.values()) {
            System.out.println(p + ": " + p.create() + ", " + p.create());
        }
        System.out.println(LOG);
    }
}
