import java.util.Arrays;

// Word boundaries, lookahead, atomic groups and possessive quantifiers in the runtime's regular
// expressions, with Java's word characters and the groups a lookahead leaves captured.
public class RegexBoundariesAndLookahead {
    public static void main(String[] args) {
        String zeros = "((?:(?:^|:)0+\\b){2,8}):?(?!\\S*\\b\\1:0+\\b)(\\S*)";
        for (String address : new String[] {"fe80:0:0:0:0:0:0:1", "1:0:0:2:0:0:0:3", "0:0:0:0:0:0:0:0"}) {
            System.out.println(address.replaceAll(zeros, "::$2"));
        }
        System.out.println(Arrays.toString("one twoéthree".split("\\b")));
        System.out.println("hello world".replaceAll("\\B", "-"));
        System.out.println("éx ́y x𝅧y".replaceAll("\\b", "|"));
        System.out.println("aaa".matches("a++a") + " " + "aab".matches("a*+b") + " " + "ab".matches("(?>a|ab)"));
        System.out.println("aaa".replaceAll("(a?)++", "<$1>") + " " + "aaab".replaceAll("(?>a|aa)b", "X"));
        System.out.println(Arrays.toString("a1b2c3".split("(?=\\d)")));
        System.out.println("foobar foobaz".replaceAll("foo(?!bar)", "X") + " " + "xy".replaceAll("(?:(?=(x))z|y)", "<$1>"));
    }
}
