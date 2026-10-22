// Field initializers that read the fields that earlier ones set, as javac's `Option.HELP_LINT`
// builds its format from its indent: each is computed before the object, after the field it
// reads.
public class InitializersReadEarlierFields {
    static final class Help {
        final String indent = "  ";
        final String keyFormat = indent + "%-" + (28 - "    ".length()) + "s %s";
        final String header = indent + indent + "options";
        final int width = keyFormat.length();

        String line(String key, String description) {
            return String.format(keyFormat, key, description);
        }
    }

    public static void main(String[] args) {
        Help help = new Help();
        System.out.println(help.header);
        System.out.println(help.line("all", "every warning"));
        System.out.println(help.width);
    }
}
