// An enum whose constructors call one another (`this(..)`) through several levels, with null
// arguments, an enum-typed field that may be null, and a varargs constructor: its fields are
// tables built by the constructors.
public enum EnumConstructorChainsAndNulls {
    HELP("--help", "opt.help", Kind.STANDARD),
    VERSION("--version", "opt.version", Kind.STANDARD, Choice.ONE),
    SOURCE("--source", "release", "opt.source", Kind.EXTENDED),
    LINT("-Xlint", "opt.lint", Kind.HIDDEN, Choice.ANY, "all", "none", "cast"),
    PLAIN(null, null, null);

    enum Kind { STANDARD, EXTENDED, HIDDEN }

    enum Choice { ONE, ANY }

    final String text;
    final String argsName;
    final String descr;
    final Kind kind;
    final Choice choice;
    final String[] choices;

    EnumConstructorChainsAndNulls(String text, String descr, Kind kind) {
        this(text, null, descr, kind);
    }

    EnumConstructorChainsAndNulls(String text, String descr, Kind kind, Choice choice) {
        this(text, null, descr, kind, choice, new String[0]);
    }

    EnumConstructorChainsAndNulls(String text, String argsName, String descr, Kind kind) {
        this(text, argsName, descr, kind, null, null);
    }

    EnumConstructorChainsAndNulls(String text, String descr, Kind kind, Choice choice, String... choices) {
        this(text, null, descr, kind, choice, choices);
    }

    private EnumConstructorChainsAndNulls(String text, String argsName, String descr, Kind kind, Choice choice,
            String[] choices) {
        this.text = text;
        this.argsName = argsName;
        this.descr = descr;
        this.kind = kind;
        this.choice = choice;
        this.choices = choices;
    }

    public static void main(String[] args) {
        for (EnumConstructorChainsAndNulls o : values()) {
            String list = o.choices == null ? "none" : String.join(",", o.choices);
            System.out.println(o + " " + o.text + " " + o.argsName + " " + o.descr + " " + o.kind + " "
                    + o.choice + " " + list + " " + (o.choice == null));
        }
    }
}
