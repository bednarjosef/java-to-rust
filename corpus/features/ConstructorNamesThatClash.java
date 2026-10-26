// Constructors whose parameter types come to the same Rust names: a `String[]` that is passed on
// as an `Object[]` is held as one, so `Names(String[], String)` and `Names(Object[], String)` take
// the same parameters (JNA's `StringArray`).
public class ConstructorNamesThatClash {
    static class Names {
        final String joined;

        Names(String[] names) {
            this(names, false);
        }

        Names(String[] names, boolean upper) {
            this((Object[]) names, upper ? "upper" : "plain");
        }

        Names(String[] names, String style) {
            this((Object[]) names, style + "!");
        }

        private Names(Object[] names, String style) {
            StringBuilder out = new StringBuilder(style).append(':');
            for (Object name : names) {
                String text = String.valueOf(name);
                out.append(' ').append(style.startsWith("upper") ? text.toUpperCase() : text);
            }
            joined = out.toString();
        }
    }

    public static void main(String[] args) {
        String[] names = {"ada", "grace"};
        System.out.println(new Names(names).joined);
        System.out.println(new Names(names, true).joined);
        System.out.println(new Names(names, "upper").joined);
        System.out.println(new Names(names, "plain").joined);
    }
}
