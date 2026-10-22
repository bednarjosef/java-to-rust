// A `?:` whose branches are an enum and a string, whose type javac leaves unnamed (the
// intersection `Serializable & Comparable<...>`): its value is an `Object`, shown in a
// concatenation as its own text.
public class ConditionalOfMixedTypes {
    enum Style { FULL, SHORT }

    static String describe(Style style, String template) {
        return "Localized(" + (template != null ? template : (style.ordinal() > 0 ? style : "") + ",") + ")";
    }

    public static void main(String[] args) {
        System.out.println(describe(Style.FULL, null));
        System.out.println(describe(Style.SHORT, null));
        System.out.println(describe(Style.SHORT, "yyyy"));
        Object either = args.length > 0 ? Style.SHORT : "none";
        System.out.println(either);
    }
}
