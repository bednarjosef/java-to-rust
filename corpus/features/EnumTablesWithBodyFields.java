// An enum whose constructor runs code (its fields are tables) and whose constants' bodies declare
// fields, which their initializers set after the constructor, read in the body's methods and
// in a lambda there.
public enum EnumTablesWithBodyFields {
    PLAIN("-a --all", null),
    LINT("-b", "x") {
        private final String INDENT = "  ";
        private final String FORMAT = INDENT + "%s=" + "%s";
        @Override
        public void process(String option) {
            System.out.println(String.format(FORMAT, option, names.length));
            java.util.function.Consumer<String> c = key -> System.out.println(String.format(FORMAT, key, 2));
            c.accept(primary);
        }
    },
    INHERIT("--c", null, 7);

    final String[] names;
    final String primary;
    final String arg;
    final int weight;

    EnumTablesWithBodyFields(String text, String arg, int weight) {
        System.out.println("building " + text);
        this.names = text.trim().split(" ");
        this.primary = names[0];
        this.arg = arg;
        this.weight = weight;
    }

    EnumTablesWithBodyFields(String text, String arg) { this(text, arg, text.length()); }

    public void process(String option) { System.out.println(primary + " " + option + " " + arg + " " + weight); }

    public static void main(String[] args) {
        System.out.println("main");
        for (EnumTablesWithBodyFields p : values()) p.process("z");
    }
}
