package factories;

/** A converter made by its static `newInstance(String[])`, as log4j's pattern converters are. */
public final class Upper implements Converter {
    private final int times;

    private Upper(String[] options) {
        // A `String[]` that is an `Object[]`, so the translation holds every `String[]` as one.
        Object[] all = options;
        this.times = all.length;
    }

    public String convert(String text) {
        return text.toUpperCase().repeat(times);
    }

    public static Upper newInstance(final String[] options) {
        return new Upper(options);
    }
}
