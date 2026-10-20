// Builds a long string by repeated concatenation, then scans it with charAt: string copying, and
// a loop over the characters of a string.
public class StringBuild {
    static String concatenated(int n) {
        String s = "";
        for (int i = 0; i < n; i++) {
            s = s + (char) ('a' + (i * 7 + i / 3) % 26);
        }
        return s;
    }

    static int countOf(String s, char target) {
        int count = 0;
        for (int i = 0; i < s.length(); i++) {
            if (s.charAt(i) == target) {
                count++;
            }
        }
        return count;
    }

    public static void main(String[] args) {
        String text = concatenated(4000);
        int total = 0;
        for (int round = 0; round < 1000; round++) {
            total += countOf(text, (char) ('a' + round % 26));
        }
        System.out.println("length " + text.length() + " first " + text.charAt(0) + " matches " + total);
    }
}
