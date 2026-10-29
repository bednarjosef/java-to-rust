import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * {@code toLowerCase(locale)} and {@code toUpperCase(locale)} of a string that may be null, inside
 * a {@code try} whose {@code catch} reads the message of the {@code NullPointerException}, as a key
 * store wraps what it catches in an exception of its own with the same message. A null locale
 * throws too, with no message; locales built or parsed map as the root locale does.
 */
public class LocaleCaseMappingInStores {
    static final Map<String, Integer> ENTRIES = new HashMap<>();

    static class StoreException extends Exception {
        StoreException(String message, Throwable cause) {
            super(message, cause);
        }
    }

    static void store(String alias, int value) throws StoreException {
        synchronized (ENTRIES) {
            try {
                ENTRIES.put(alias.toLowerCase(Locale.ENGLISH), value);
            } catch (Exception e) {
                throw new StoreException(e.getMessage(), e);
            }
        }
    }

    static String upper(String name, Locale locale) {
        try {
            String s = name.toUpperCase(locale);
            return s.startsWith("CPU") && s.length() > 3 ? s : "-";
        } catch (NullPointerException e) {
            return "npe " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        String missing = args.length > 9 ? "Z" : null;
        List<String> aliases = new ArrayList<>(List.of("Alpha", "BETA"));
        aliases.add(1, missing);
        for (String alias : aliases) {
            try {
                store(alias, alias == null ? 0 : alias.length());
                System.out.println("stored " + alias);
            } catch (StoreException e) {
                System.out.println("store failed: " + e.getMessage() + " / " + e.getCause().getClass().getName());
            }
        }
        System.out.println(new java.util.TreeMap<>(ENTRIES));
        Locale built = new Locale.Builder().setLanguage("fr").setRegion("CA").build();
        Locale tagged = Locale.forLanguageTag(args.length > 3 ? "de-AT" : "en-GB");
        Locale none = args.length > 9 ? Locale.ROOT : null;
        for (Locale locale : new Locale[] {Locale.ROOT, built, tagged, none}) {
            System.out.println(upper("cpu0", locale) + " " + upper("gpu", locale) + " " + upper(missing, locale));
        }
    }
}
