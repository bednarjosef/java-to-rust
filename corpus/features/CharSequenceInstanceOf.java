// `instanceof String` and `instanceof StringBuilder` on a `CharSequence`, which holds either, with
// and without a binding.
public class CharSequenceInstanceOf {
    static String kind(CharSequence text) {
        if (text instanceof String) {
            return "string of " + text.length();
        }
        if (text instanceof StringBuilder b) {
            b.append('!');
            return "builder " + b;
        }
        return "other";
    }

    static String normalize(CharSequence src) {
        if (src instanceof String s && s.isEmpty()) {
            return "(empty)";
        }
        return src.toString().trim();
    }

    public static void main(String[] args) {
        System.out.println(kind("abc"));
        System.out.println(kind(new StringBuilder("xy")));
        System.out.println(normalize(""));
        System.out.println(normalize("  padded  "));
        System.out.println(normalize(new StringBuilder(" built ")));
    }
}
