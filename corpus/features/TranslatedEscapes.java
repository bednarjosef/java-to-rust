// `String.translateEscapes`, which javac's tokenizer runs on a text block: the escapes of a
// string literal, read from text, and the failure of one that Java does not have, caught as the
// failure of `repeat` with a negative count is.
public class TranslatedEscapes {
    static String show(String s) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c < ' ' || c == 127) {
                sb.append("<").append((int) c).append(">");
            } else {
                sb.append(c);
            }
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        String[] inputs = {
            "plain", "a\\tb\\nc", "\\b\\f\\r\\s\\'\\\"\\\\", "\\101\\60\\0x\\377\\4000\\12",
            "joined \\\nline", "crlf \\\r\nline", "", "\\u0041 stays",
        };
        for (String in : inputs) {
            System.out.println(show(in.translateEscapes()));
        }
        for (String bad : new String[] {"\\q", "end\\", "\\é"}) {
            try {
                System.out.println(bad.translateEscapes());
            } catch (IllegalArgumentException e) {
                System.out.println(show(e.getMessage()));
            }
        }
        try {
            System.out.println("ab".repeat(args.length - 2));
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
    }
}
