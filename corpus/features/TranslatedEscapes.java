// String.translateEscapes(): the escape sequences of a string become the characters they stand
// for, as in a text block: letter escapes, octal escapes of one to three digits (a first digit
// above 3 takes at most two, so `\400` is a space and a `0`), `\s`, a backslash before a line end
// (a `\r\n` is one terminator, dropped with the backslash), and a doubled backslash. A malformed
// escape throws IllegalArgumentException, caught here and its message printed.
public class TranslatedEscapes {
    // The translation as its length and code points, so that invisible characters show.
    static void show(String label, String s) {
        StringBuilder points = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            points.append(i == 0 ? "" : " ").append((int) s.charAt(i));
        }
        System.out.println(label + " " + s.length() + ": " + points);
    }

    static void translate(String label, String s) {
        try {
            show(label, s.translateEscapes());
        } catch (IllegalArgumentException e) {
            System.out.println(label + " threw " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        translate("letters", "\\b\\f\\n\\r\\s\\t\\'\\\"\\\\");
        translate("octal", "\\7\\77\\101\\377\\0\\08");
        translate("wide", "\\400|\\7777");
        translate("lines", "a\\\nb\\\r\nc\\\rd");
        translate("plain", "plain text, no escapes");
        translate("empty", "");
        translate("doubled", "\\\\n");
        translate("tail", "ab\\\n");
        translate("tabs", "\\t\\t\\s");
        // A backslash before `u` is no escape, so these throw, and the catch prints the message.
        translate("unicode", "x\\" + "u0041");
        translate("letter", "\\q");
        translate("trailing", "ab\\");
        System.out.println("done");
    }
}
