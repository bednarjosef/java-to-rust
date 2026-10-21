import java.util.Locale;

// String.toUpperCase(locale) and toLowerCase(locale) read the locale's language, so a null locale
// is a NullPointerException, after the receiver is evaluated.
public class CaseInLocale {
    // Never assigned: no code here initializes Locale.
    static Locale none;

    static String upper(String s, Locale locale) {
        return s.toUpperCase(locale);
    }

    static String lower(String s, Locale locale) {
        return s.toLowerCase(locale);
    }

    static String text(String s) {
        System.out.println("receiver " + s);
        return s;
    }

    public static void main(String[] args) {
        try {
            System.out.println(upper("x", null));
        } catch (NullPointerException e) {
            System.out.println("no locale to upper-case in");
        }
        try {
            System.out.println(text("Y").toLowerCase(none));
        } catch (NullPointerException e) {
            System.out.println("no locale to lower-case in");
        }
        try {
            System.out.println(lower(null, null));
        } catch (NullPointerException e) {
            System.out.println("no string");
        }
    }
}
