// A constant body reads a table field of another enum, a field of its own body, and a method that
// only its body overrides; the enum's own methods read the table of the constant they are called on.
public class EnumBodyForeignField {
    enum Category {
        IDENTITY("identity", "id"),
        PATHS("paths");

        final String option;
        final String[] names;

        Category(String option, String... aliases) {
            this.option = option;
            this.names = new String[aliases.length + 1];
            names[0] = option;
            for (int i = 0; i < aliases.length; i++) {
                names[i + 1] = aliases[i];
            }
        }
    }

    interface Joiner {
        String join(String s);
    }

    enum Opt {
        PLAIN,
        HELP {
            private final String INDENT = "  " + "  ";
            private final String FORMAT = INDENT + "%-" + (12 - INDENT.length()) + "s %s";

            @Override
            String describe(String key) {
                return String.format(FORMAT, key, Category.IDENTITY.names[0]) + "|" + Category.PATHS.option;
            }

            @Override
            String joined() {
                Joiner j = s -> FORMAT + s + Category.IDENTITY.option;
                return j.join("q");
            }
        };

        String describe(String key) {
            return "plain " + key + " " + name();
        }

        String joined() {
            return "none";
        }
    }

    public static void main(String[] args) {
        for (Opt o : Opt.values()) {
            System.out.println(o.describe("key") + " / " + o.joined());
        }
        System.out.println(Category.IDENTITY.names.length + " " + Category.PATHS.names[0]);
        Opt h = Opt.HELP;
        System.out.println(h.describe("x") + " " + h.ordinal());
    }
}
