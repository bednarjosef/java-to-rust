// Iterating Set.of(...) and building locales, whose JDK code has NullPointerException messages
// on a field of an outer instance (ImmutableCollections.SetN's iterator), on a dereferenced
// parameter (LocaleUtils.caseIgnoreMatch) and on an enum's fields kept as tables (StreamOpFlag).
import java.util.Locale;
import java.util.Set;

public class LocalesAndImmutableSets {
    public static void main(String[] args) {
        try {
            Set<String> set = Set.of("a", "bb", "ccc");
            int total = 0;
            for (String s : set) {
                total += s.length();
            }
            Locale locale = Locale.forLanguageTag("de-DE");
            Locale other = new Locale.Builder().setLanguage("de").setRegion("DE").build();
            System.out.println(total + " " + locale + " " + locale.equals(other) + " " + locale.getLanguage());
            System.out.println(Set.of(1, 2, 3).contains(2) + " " + Locale.forLanguageTag("fr-CA").getCountry());
        } catch (RuntimeException e) {
            System.out.println(e.getMessage());
        }
    }
}
