// A conditional of a string and null returned as a CharSequence, as log4j's
// PropertyFilePropertySource.getNormalForm returns its property name.
public class CharSequenceOrNull {
    static final String PREFIX = "log4j2.";
    static CharSequence normalForm(StringBuilder camelCase) {
        return camelCase.length() > 0 ? PREFIX + camelCase : null;
    }
    static CharSequence other(String s) {
        return s.isEmpty() ? new StringBuilder("empty") : s;
    }
    public static void main(String[] args) {
        System.out.println(normalForm(new StringBuilder("fooBar")));
        System.out.println(normalForm(new StringBuilder()));
        System.out.println(other("") + " " + other("x"));
    }
}
