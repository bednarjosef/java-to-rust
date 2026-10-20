// split, replaceAll, replaceFirst and matches with the regular expressions the translation
// supports: literals, escapes, classes, groups, alternation and quantifiers. split drops trailing
// empty strings unless the limit is negative, and a match of nothing at the start leaves no empty
// first string. The last line splits with a regular expression that is only known when the
// program runs, and is malformed, which throws PatternSyntaxException.
public class RegexSplit {
    static void show(String[] parts) {
        StringBuilder sb = new StringBuilder();
        sb.append(parts.length).append(':');
        for (String p : parts) {
            sb.append(" [").append(p).append(']');
        }
        System.out.println(sb);
    }

    static int sumOf(String csv) {
        int sum = 0;
        for (String field : csv.split(",")) {
            sum += Integer.parseInt(field.trim());
        }
        return sum;
    }

    public static void main(String[] args) {
        show("a,b,,c,,".split(","));
        show("a,b,,c,,".split(",", -1));
        show("a,b,,c,,".split(",", 2));
        show("".split(","));
        show(",a".split(","));
        show("hello".split(""));
        show("  the quick   brown fox ".split("\\s+"));
        show("  the quick   brown fox ".trim().split(" +"));
        show("1.2.3".split("\\."));
        show("a1b22c333d".split("[0-9]+"));
        show("key=value; other = thing".split("\\s*[;=]\\s*"));
        show("x|y|z".split("\\|"));
        show("one, two;three".split("[,;] ?"));
        show("aXbYc".split("X|Y"));
        show("tab\tseparated\tvalues".split("\t"));
        show("path/to/file.txt".split("/", 0));
        show("abcabc".split("(b)(c)"));
        show("é,ü,😀".split(","));
        System.out.println(sumOf("1, 2, 3, 40"));

        System.out.println("a1b2c3".replaceAll("[0-9]", "#") + " " + "a1b2c3".replaceFirst("\\d", "#"));
        System.out.println("John Smith".replaceAll("(\\w+) (\\w+)", "$2, $1"));
        System.out.println("aaa".replaceAll("a*", "X") + " " + "abc".replaceAll("", "-"));
        System.out.println("  many   spaces  ".replaceAll("\\s+", " ") + "|" + "x.y.z".replaceAll("\\.", "\\$"));
        System.out.println("2024-01-15".matches("\\d{4}-\\d{2}-\\d{2}") + " " + "2024-1-15".matches("\\d{4}-\\d{2}-\\d{2}"));
        System.out.println("abc".matches("[a-c]+") + " " + "abcd".matches("[a-c]+") + " " + "".matches("a?"));
        System.out.println("Hello".matches("(?:H|J)ello") + " " + "colour".matches("colou?r") + " " + "aaab".matches("a{2,}b"));
        System.out.println("a.b".matches("a.b") + " " + "a\nb".matches("a.b") + " " + "AB_9".matches("\\w+"));

        String separator = args.length > 0 ? args[0] : "(";
        show("a(b".split(separator));
    }
}
