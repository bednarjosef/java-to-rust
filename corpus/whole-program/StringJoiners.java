import java.util.StringJoiner;

// The JDK's `StringJoiner`, whose `CharSequence` parameters are strings and builders: calls of
// `CharSequence`'s methods on them run the string's or the builder's.
public class StringJoiners {
    static int total(CharSequence... parts) {
        int n = 0;
        for (CharSequence part : parts) {
            n += part.length();
        }
        return n;
    }

    public static void main(String[] args) {
        StringJoiner joiner = new StringJoiner(", ", "[", "]");
        System.out.println(joiner + " " + joiner.length());
        joiner.setEmptyValue("nothing");
        System.out.println(joiner);
        joiner.add("one").add(new StringBuilder("tw").append('o')).add(null);
        System.out.println(joiner + " " + joiner.length());
        StringJoiner other = new StringJoiner("-");
        other.add("x").add("y").add("z");
        joiner.merge(other);
        System.out.println(joiner);
        System.out.println(other + " " + other.length());
        CharSequence text = new StringBuilder("abc");
        System.out.println(text.toString() + text.charAt(1) + " " + total("ab", new StringBuilder("cde"), text));
    }
}
