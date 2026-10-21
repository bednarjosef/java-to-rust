import java.util.Arrays;

public class RegexLineBreaksAndBackReferences {
    static String show(String[] parts) {
        StringBuilder sb = new StringBuilder();
        for (String p : parts) {
            sb.append('[').append(p.replace("\r", "\\r").replace("\n", "\\n")).append(']');
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        String text = "one\r\ntwo\nthree\rfour\u2028five\u000Bsix\r\n\r\nseven";
        System.out.println(show(text.split("\\R")));
        System.out.println(show(text.split("\\R", -1)));
        System.out.println("a\r\n\nb".split("\\R+").length);
        System.out.println("x\r\ny".replaceAll("\\R", "|"));
        System.out.println("\r\n".matches("\\R") + " " + "\r\n".matches("\\R\\n") + " " + "\n\r".matches("\\R"));
        String path = "a..b...c.d";
        System.out.println(path.replaceAll("(\\.)(\\1)+", "/"));
        System.out.println("abab cdcd efef xyxz".replaceAll("(\\w)(\\w)\\1\\2", "<$2$1>"));
        System.out.println("hello  world  ".replaceAll("( )\\1+", "_"));
        System.out.println("aa1".matches("(a)\\11") + " " + "a".matches("\\2(a)") + " " + "abcabc".matches("(a(b)(c))\\1"));
        System.out.println(Arrays.toString("x--y==z".split("([-=])\\1")));
        String dynamic = args.length > 5 ? "x" : "(\\d)\\1";
        System.out.println("1122334".replaceAll(dynamic, "#"));
    }
}
