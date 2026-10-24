package java.lang;

import java.util.Comparator;

/**
 * The static fields of {@code String} that are objects, which the translator reads here because
 * its strings are the runtime's and {@code String} is not translated (see
 * {@code program::redirected_field}).
 */
final class TranslatedStrings {
    /** {@code String.CASE_INSENSITIVE_ORDER}: one comparator, as the JDK has. */
    static final Comparator<String> CASE_INSENSITIVE_ORDER = new CaseInsensitiveComparator();

    private TranslatedStrings() {}

    /**
     * Compares as {@code compareToIgnoreCase} does, which the JDK writes as this comparison. The
     * JDK's first reads the text of each string, so a null one throws as reading its field does.
     */
    private static final class CaseInsensitiveComparator implements Comparator<String>, java.io.Serializable {
        public int compare(String s1, String s2) {
            if (s1 == null) {
                throw new NullPointerException("Cannot read field \"value\" because \"s1\" is null");
            }
            if (s2 == null) {
                throw new NullPointerException("Cannot read field \"value\" because \"s2\" is null");
            }
            return s1.compareToIgnoreCase(s2);
        }
    }
}
