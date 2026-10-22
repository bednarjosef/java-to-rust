import java.util.Locale;

// The NullPointerException of toUpperCase and toLowerCase with a locale on a null string, whose
// message a catch reads, as the JDK's CurrencyNameProviderImpl.getSymbol does.
public class CaseInLocaleNullMessages {
    static String code(int i) {
        return i < 0 ? null : "eur" + i;
    }

    static String upper(int i) {
        try {
            return code(i).toUpperCase(Locale.ROOT);
        } catch (NullPointerException e) {
            return "failed: " + e.getMessage();
        }
    }

    static String lower(int i) {
        String s = code(i);
        try {
            return s.toLowerCase(Locale.ROOT);
        } catch (NullPointerException e) {
            return "failed: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(upper(1));
        System.out.println(upper(-1));
        System.out.println(lower(2));
        System.out.println(lower(-1));
    }
}
