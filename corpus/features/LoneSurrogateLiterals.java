// String literals that hold a surrogate without its other half: their code units, joining halves
// into a pair, matching them in a switch, and a static that holds one.
public class LoneSurrogateLiterals {
    static final String HIGH = "\uD83D";
    static String low = "\uDE00";

    static String kind(String s) {
        switch (s) {
            case "\uD800":
                return "lone high";
            case "\uDC00" + "x":
                return "lone low then x";
            case "a":
                return "a";
            default:
                return "other";
        }
    }

    static String units(String s) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            if (i > 0) {
                sb.append(' ');
            }
            sb.append(Integer.toHexString(s.charAt(i)));
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        String pair = HIGH + low;
        System.out.println(pair + " " + pair.length() + " " + pair.codePointAt(0));
        System.out.println(units("a\uD800b") + " / " + "a\uD800b".length());
        System.out.println(kind("\uD800") + ", " + kind("\uDC00" + "x") + ", " + kind("a") + ", " + kind("𐀀"));
        System.out.println("\uD800".equals(String.valueOf('\uD800')) + " " + "\uDBFF".compareTo("") + " " + "\uD800".hashCode());
        String built = "x\uD83D";
        built += "\uDE00y";
        System.out.println(built + " " + built.length() + " " + units(built));
        System.out.println("lone: " + "\uD800" + "|" + "\uDFFF".toUpperCase().length());
        String split = "😀".substring(0, 1);
        System.out.println(split.equals(HIGH) + " " + split.equals("\uD83D") + " " + units(split + low));
        System.out.println(("\uD800" + 1).length() + " " + "\uD800".isEmpty() + " " + "x\uDC00".indexOf("\uDC00"));
        StringBuilder sb = new StringBuilder("\uD83D");
        sb.append("\uDE00");
        System.out.println(sb + " " + sb.length() + " " + sb.toString().codePointCount(0, sb.length()));
    }
}
