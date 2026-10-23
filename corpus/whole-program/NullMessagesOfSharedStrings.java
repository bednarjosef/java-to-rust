// The null messages of string methods where every string is shared (the program compares strings
// by identity): `toLowerCase` and `toUpperCase` of a null locale, whose receiver and arguments
// are held twice, once for Java's order of evaluation and once to keep the receiver, throw
// without a message as the JDK's code does, and a null receiver is named.
import java.util.Locale;

public class NullMessagesOfSharedStrings {
    static Locale locale(int i) {
        return i > 100 ? Locale.US : null;
    }

    static String text(int i) {
        return i > 100 ? "Abc" : null;
    }

    static boolean same(String a, String b) {
        return a == b;
    }

    public static void main(String[] args) {
        String kept = "Abc".toLowerCase(Locale.US);
        System.out.println(same(kept, args.length > 3 ? "abc" : kept) + " " + same("abc".toLowerCase(Locale.US), "abc"));
        for (int which = 0; which < 4; which++) {
            try {
                switch (which) {
                    case 0 -> System.out.println("Abc".toLowerCase(locale(which)));
                    case 1 -> System.out.println("Abc".toUpperCase(locale(which)));
                    case 2 -> System.out.println(text(which).toLowerCase(Locale.ROOT));
                    default -> System.out.println(text(which).toUpperCase(locale(200)));
                }
            } catch (NullPointerException e) {
                System.out.println(which + ": " + e.getMessage());
            }
        }
    }
}
