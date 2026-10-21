// The variable of a for-each loop over strings counts UTF-16 units, whatever the array holds:
// non-ASCII letters, an emoji, strings stored later, rows of a grid of strings.
public class StringArrayForEachLength {
    static String[] words = {"a", "Q", "é"};

    static int total(String[] p) {
        int n = 0;
        for (String s : p) {
            n += s.length();
        }
        return n;
    }

    public static void main(String[] args) {
        String out = "";
        for (String w : words) {
            out += w.length() + ",";
        }
        System.out.println(out);
        String[] local = {"plain", "naïve", "😀!"};
        for (String s : local) {
            System.out.println(s + " " + s.length() + " " + s.charAt(s.length() - 1));
        }
        String[] later = new String[] {"x", "y"};
        later[1] = "日本語";
        System.out.println(total(later) + " " + total(local));
        String[][] grid = {{"ab", "ß"}, {"€uro"}};
        for (String[] row : grid) {
            for (String s : row) {
                System.out.print(s.length() + " " + s.indexOf('u') + ";");
            }
        }
        System.out.println();
    }
}
