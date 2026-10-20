public class Chars {
    static char shift(char c, int by) {
        return (char) (c + by);
    }

    static int digitValue(char c) {
        return c - '0';
    }

    static boolean isVowel(char c) {
        char lower = Character.toLowerCase(c);
        return lower == 'a' || lower == 'e' || lower == 'i' || lower == 'o' || lower == 'u';
    }

    static void describe(char c) {
        System.out.println(c + ": digit " + Character.isDigit(c) + ", letter " + Character.isLetter(c)
            + ", upper " + Character.isUpperCase(c) + ", lower " + Character.isLowerCase(c)
            + ", " + Character.toUpperCase(c) + Character.toLowerCase(c) + " " + (int) c);
    }

    static String reverse(String s) {
        String out = "";
        for (int i = s.length() - 1; i >= 0; i--) {
            out += s.charAt(i);
        }
        return out;
    }

    public static void main(String[] args) {
        char a = 'a';
        char newline = '\n';
        char quote = '\'';
        char backslash = '\\';
        char eAcute = 'é';
        System.out.println(a);
        System.out.print(newline);
        System.out.println("" + quote + backslash + eAcute + '"' + '{' + '}');
        System.out.println(a + 1);
        System.out.println((char) (a + 1));
        System.out.println(a + 'b');
        System.out.println("" + a + 'b');
        System.out.println('z' - a);
        System.out.println(shift('a', 25) + " " + shift('A', -1));
        System.out.println(digitValue('7') * 3);

        // Wrapping at the ends of the type.
        char max = Character.MAX_VALUE;
        char min = Character.MIN_VALUE;
        max++;
        min--;
        System.out.println((int) max + " " + (int) min);
        char c = 65535;
        c += 2;
        System.out.println((int) c);
        c -= 3;
        System.out.println((int) c);
        System.out.println((int) (char) -1 + " " + (int) (char) 65536 + " " + (int) (char) 70000);
        System.out.println((int) (char) (byte) -1 + " " + (int) (char) 3.7 + " " + (int) (char) -2.5);
        System.out.println((int) (char) 1e10 + " " + (int) (char) Double.NaN);
        System.out.println(-a + " " + ~a + " " + (a << 2) + " " + (a >> 1) + " " + (char) (a ^ ' '));

        // Comparisons and counting loops over chars.
        int count = 0;
        for (char x = 'a'; x < 'z'; x++) {
            count++;
        }
        System.out.println(count);
        for (char x = 'A'; x <= 'F'; x += 2) {
            System.out.print(x);
        }
        System.out.println();
        char m = 'm';
        System.out.println((m > 'a') + " " + (m == 109) + " " + (m != 'm') + " " + (m < -1) + " " + (m >= 'n'));
        char ch = m;
        ch++;
        ch = (char) (ch * 2);
        System.out.println((int) ch + " " + ch);

        // Character methods on ASCII and Latin-1.
        describe('a');
        describe('Z');
        describe('5');
        describe(' ');
        describe('é');
        describe('ß');
        describe('ÿ');
        describe('µ');
        describe('ª');
        describe('×');
        describe('²');
        System.out.println(isVowel('E') + " " + isVowel('x'));

        // charAt, by UTF-16 code unit.
        String word = "Hello, Wörld";
        int upper = 0;
        int letters = 0;
        for (int i = 0; i < word.length(); i++) {
            char w = word.charAt(i);
            if (Character.isUpperCase(w)) {
                upper++;
            }
            if (Character.isLetter(w)) {
                letters++;
            }
        }
        System.out.println(upper + " " + letters + " " + word.charAt(8) + " " + reverse(word));
        String ascii = "abc";
        int sum = 0;
        for (int i = 0; i < ascii.length(); i++) {
            sum += ascii.charAt(i);
        }
        System.out.println(sum + " " + ascii.charAt(2));
        String emoji = "a😀b";
        System.out.println(emoji.length() + " " + (int) emoji.charAt(1) + " " + (int) emoji.charAt(2) + " " + emoji.charAt(3));
        String s = "";
        s += 'x';
        s += a;
        s += (char) 66;
        System.out.println(s + " " + s.length());
        System.out.println(ascii.charAt(3));
    }
}
