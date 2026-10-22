package jdk.internal.util;

/**
 * The translator's patch of the JDK's {@code Exceptions} (see docs/jdk-overlays.md).
 * {@code setup} reads the categories of {@code jdk.includeInExceptions} without waiting for
 * {@code VM.isBooted}, which stays false in a translation that does not go through the JVM's
 * boot, and {@code formatMsg} puts its strings in for the {@code %s} of its format itself
 * ({@code formatStrings}), refusing any other conversion with an
 * {@code UnsupportedOperationException}: the failures of {@code String.format} are not named,
 * which the {@code catch} clauses around the JDK's parsers would have to be checked against.
 */
class Exceptions {
    @Edit(find = "String.format(format, (Object[])args)", replace = "formatStrings(format, args)")
    public static String formatMsg(String format, SensitiveInfo... infos);

    /**
     * {@code String.format(format, args)} for the formats that the JDK gives {@code formatMsg},
     * whose only conversions are {@code %s} of strings: one through {@code String.format} may
     * throw what the runtime does not name, which the {@code catch} clauses around the JDK's
     * parsers would have to be checked against. Any other conversion is not translated.
     */
    @Add
    private static String formatStrings(String format, String[] args) {
        StringBuilder sb = new StringBuilder();
        int next = 0;
        int i = 0;
        while (i < format.length()) {
            char c = format.charAt(i);
            if (c != '%') {
                sb.append(c);
                i++;
                continue;
            }
            if (!format.startsWith("%s", i) || next >= args.length) {
                throw new UnsupportedOperationException("formatMsg(\"" + format + "\")");
            }
            sb.append(args[next]);
            next++;
            i += 2;
        }
        return sb.toString();
    }

    // A translated program runs after the JVM's boot, which the translation does not go through
    // (its `VM.isBooted` stays false), so the categories are read on the first call. The class of
    // `SecurityProperties.INCLUDE_JAR_NAME_IN_EXCEPTIONS` is not translated: only its
    // `includedInExceptions` is answered.
    @Edit(find = "initialized || !VM.isBooted()", replace = "initialized")
    @Edit(find = "SecurityProperties.INCLUDE_JAR_NAME_IN_EXCEPTIONS",
            replace = "SecurityProperties.includedInExceptions(\"jar\")")
    public static void setup();
}
