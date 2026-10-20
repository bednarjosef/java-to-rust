public class TextToolkit {
    static String runLengthEncode(String s) {
        if (s.length() == 0) {
            return "";
        }
        String out = "";
        int run = 1;
        for (int i = 1; i <= s.length(); i++) {
            if (i < s.length() && s.charAt(i) == s.charAt(i - 1)) {
                run++;
            } else {
                out = out + run + s.charAt(i - 1);
                run = 1;
            }
        }
        return out;
    }

    static String runLengthDecode(String s) {
        String out = "";
        int count = 0;
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c >= '0' && c <= '9') {
                count = count * 10 + (c - '0');
            } else {
                for (int k = 0; k < count; k++) {
                    out = out + c;
                }
                count = 0;
            }
        }
        return out;
    }

    static char shift(char c, int amount) {
        if (c >= 'a' && c <= 'z') {
            return (char) ('a' + (c - 'a' + amount + 26) % 26);
        }
        if (c >= 'A' && c <= 'Z') {
            return (char) ('A' + (c - 'A' + amount + 26) % 26);
        }
        return c;
    }

    static String caesar(String s, int amount) {
        String out = "";
        for (int i = 0; i < s.length(); i++) {
            out = out + shift(s.charAt(i), amount);
        }
        return out;
    }

    static boolean isPalindrome(String s) {
        int i = 0;
        int j = s.length() - 1;
        while (i < j) {
            char a = Character.toLowerCase(s.charAt(i));
            char b = Character.toLowerCase(s.charAt(j));
            if (!(Character.isLetter(s.charAt(i)) || Character.isDigit(s.charAt(i)))) {
                i++;
            } else if (!(Character.isLetter(s.charAt(j)) || Character.isDigit(s.charAt(j)))) {
                j--;
            } else if (a != b) {
                return false;
            } else {
                i++;
                j--;
            }
        }
        return true;
    }

    static String longestPalindrome(String s) {
        int bestStart = 0;
        int bestLength = s.length() == 0 ? 0 : 1;
        for (int center = 0; center < s.length(); center++) {
            for (int parity = 0; parity < 2; parity++) {
                int lo = center;
                int hi = center + parity;
                while (lo >= 0 && hi < s.length() && s.charAt(lo) == s.charAt(hi)) {
                    lo--;
                    hi++;
                }
                int length = hi - lo - 1;
                if (length > bestLength) {
                    bestLength = length;
                    bestStart = lo + 1;
                }
            }
        }
        String out = "";
        for (int i = bestStart; i < bestStart + bestLength; i++) {
            out = out + s.charAt(i);
        }
        return out;
    }

    /** Word lengths of a text, split on spaces and punctuation, counted into an array. */
    static int[] wordLengths(String text) {
        int[] counts = new int[20];
        int current = 0;
        for (int i = 0; i <= text.length(); i++) {
            boolean letter = i < text.length() && Character.isLetter(text.charAt(i));
            if (letter) {
                current++;
            } else {
                if (current > 0) {
                    counts[Math.min(current, 19)]++;
                }
                current = 0;
            }
        }
        return counts;
    }

    static int countVowels(String s) {
        int n = 0;
        for (int i = 0; i < s.length(); i++) {
            switch (Character.toLowerCase(s.charAt(i))) {
                case 'a':
                case 'e':
                case 'i':
                case 'o':
                case 'u':
                    n++;
                    break;
                default:
                    break;
            }
        }
        return n;
    }

    static String reverse(String s) {
        String out = "";
        for (int i = s.length() - 1; i >= 0; i--) {
            out = out + s.charAt(i);
        }
        return out;
    }

    public static void main(String[] args) {
        String[] samples = {"aaabccddddd", "abc", "", "zzzzzzzzzzzzz", "aabbaa"};
        for (String s : samples) {
            String enc = runLengthEncode(s);
            System.out.println("\"" + s + "\" -> \"" + enc + "\" -> \"" + runLengthDecode(enc) + "\"");
        }

        String message = "Attack at Dawn, Zebra!";
        String secret = caesar(message, 3);
        System.out.println(secret);
        System.out.println(caesar(secret, -3));
        System.out.println(caesar(message, 13));
        System.out.println(caesar(caesar(message, 13), 13).equals(message));

        String[] candidates = {"A man, a plan, a canal: Panama", "race a car", "No lemon, no melon", "x"};
        for (String c : candidates) {
            System.out.println(isPalindrome(c) + " " + c.length() + " " + longestPalindrome(c));
        }
        System.out.println("longest in banana: " + longestPalindrome("forgeeksskeegfor"));

        String text = "The quick brown fox jumps over the lazy dog, and then it sleeps "
                + "soundly until the sun rises again over the hills.";
        int[] lengths = wordLengths(text);
        for (int len = 1; len < lengths.length; len++) {
            if (lengths[len] > 0) {
                System.out.println(len + " letters: " + lengths[len]);
            }
        }
        System.out.println("vowels: " + countVowels(text));
        System.out.println("reversed: " + reverse("stressed"));
        System.out.println("upper: " + Character.toUpperCase('q') + Character.toUpperCase('1') + "!");
    }
}
