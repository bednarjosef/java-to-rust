public class SystemProperties {
    // A property value as a program would print it: quoted when present, `null` when not.
    static String quoted(String value) {
        return value == null ? "null" : "\"" + value + "\"";
    }

    static boolean present(String value) {
        return value != null && value.length() > 0;
    }

    public static void main(String[] args) {
        // Values fixed by the platform, not by the machine.
        System.out.println("line.separator: " + System.getProperty("line.separator").length());
        System.out.println("file.separator: " + System.getProperty("file.separator"));
        System.out.println("path.separator: " + System.getProperty("path.separator"));
        System.out.println("java.io.tmpdir: " + System.getProperty("java.io.tmpdir"));
        System.out.println("os.name: " + System.getProperty("os.name"));
        System.out.println("os.arch: " + System.getProperty("os.arch"));
        System.out.println("line.separator matches lineSeparator: "
                + System.getProperty("line.separator").equals(System.lineSeparator()));

        // Values that depend on the environment: only whether they are set.
        System.out.println("user.dir set: " + present(System.getProperty("user.dir")));
        System.out.println("user.home set: " + present(System.getProperty("user.home")));
        System.out.println("user.name set: " + present(System.getProperty("user.name")));

        // A key the runtime does not know gives null, and a default stands in for it.
        System.out.println("missing is null: " + (System.getProperty("no.such.key") == null));
        System.out.println("missing: " + quoted(System.getProperty("no.such.key")));
        System.out.println("missing, default: " + quoted(System.getProperty("no.such.key", "fallback")));
        System.out.println("missing, null default: " + quoted(System.getProperty("no.such.key", null)));
        String kept = "kept";
        System.out.println("missing, variable default: " + quoted(System.getProperty("no.such.key", kept)));

        // The default of a key that is set is not used.
        System.out.println("set, default ignored: " + quoted(System.getProperty("file.separator", "?")));

        // `Boolean.getBoolean` is true only for a property set to "true", which only `-D` does.
        String flag = "jdk.compat.DecimalFormat";
        System.out.println("getBoolean: " + Boolean.getBoolean("no.such.flag") + " "
                + Boolean.getBoolean(flag) + " " + Boolean.getBoolean("os.name") + " "
                + Boolean.getBoolean(null) + " " + Boolean.getBoolean(quoted(flag)));
    }
}
