// Strings built from `(char)` casts. A cast whose integer value the range analysis keeps within
// ASCII makes the string ASCII, so charAt and length index bytes; any other cast keeps UTF-16
// units, and the output must count them as Java does.
public class AsciiCharAt {
    static String letters(int n) {
        String s = "";
        for (int i = 0; i < n; i++) {
            s = s + (char) ('a' + (i * 5 + i / 2) % 26);
        }
        return s;
    }

    static String mixed(int n) {
        String s = "";
        for (int i = 0; i < n; i++) {
            s = s + (char) (i % 2 == 0 ? 'x' : 0xE9 + i % 3);
        }
        return s;
    }

    static int count(String s, char target) {
        int count = 0;
        for (int i = 0; i < s.length(); i++) {
            if (s.charAt(i) == target) {
                count++;
            }
        }
        return count;
    }

    public static void main(String[] args) {
        String a = letters(300);
        System.out.println(a.length() + " " + a.charAt(0) + a.charAt(299) + " " + count(a, 'a'));
        String m = mixed(9);
        System.out.println(m.length() + " " + (int) m.charAt(1) + " " + count(m, 'x') + " " + (int) m.charAt(2));
        char wide = (char) (200 + a.length() % 50);
        String w = "" + wide + a.substring(0, 3);
        System.out.println(w.length() + " " + (int) w.charAt(0) + " " + w.charAt(3));
        String small = "";
        for (int i = 0; i < 5; i++) {
            small += (char) (i * 20 + 40);
        }
        System.out.println(small.length() + " " + small.charAt(4) + " " + (int) small.charAt(0));
        System.out.println(a.charAt(a.length()));
    }
}
