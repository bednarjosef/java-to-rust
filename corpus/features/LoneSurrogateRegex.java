// Regular expressions over strings that hold a lone surrogate, which Java matches as a code
// point of its own.
public class LoneSurrogateRegex {
    static String units(String s) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            sb.append(i == 0 ? "" : " ").append(Integer.toHexString(s.charAt(i)));
        }
        return "[" + sb + "]";
    }

    public static void main(String[] args) {
        String s = "a" + "😀,b".substring(0, 1) + "," + "😀".substring(1) + ",b";
        for (String part : s.split(",")) {
            System.out.print(units(part) + " ");
        }
        System.out.println();
        System.out.println(s.split("[^,]").length + " " + s.split("-").length + " " + s.split("").length);
        System.out.println(s.matches(".+") + " " + s.matches("[\\w,]+") + " " + s.matches("a.,.,b"));
        String joined = s.replaceAll(",", "");
        System.out.println(units(joined) + " " + joined.codePointCount(0, joined.length()) + " " + joined);
        System.out.println(units(s.replaceFirst("a.", "<$0>")) + " " + units(s.replaceAll("(.),", "$1;")));
        System.out.println(units("x\uD800y".replace("x", "")) + " " + "\uD800".matches("."));
    }
}
