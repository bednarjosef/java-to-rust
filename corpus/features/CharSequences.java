// `CharSequence` as a type: strings and builders are its values, and its `length`, `charAt`,
// `isEmpty` and `toString` run the string's or the builder's. A builder stays the same object, so
// what is appended to it later shows through the `CharSequence`.
public class CharSequences {
    static String describe(CharSequence text) {
        if (text == null) {
            return "none";
        }
        StringBuilder out = new StringBuilder();
        for (int i = text.length() - 1; i >= 0; i--) {
            out.append(text.charAt(i));
        }
        return text + " " + text.length() + " " + text.isEmpty() + " " + out + " " + text.toString().toUpperCase();
    }

    public static void main(String[] args) {
        StringBuilder builder = new StringBuilder("buil");
        CharSequence[] all = {"plain", builder, "", null};
        builder.append("der");
        for (CharSequence c : all) {
            System.out.println(describe(c));
        }
        Object o = all[1];
        System.out.println(o instanceof CharSequence cs ? "sequence of " + cs.length() : "other");
        CharSequence again = (CharSequence) o;
        System.out.println(again == builder);
    }
}
