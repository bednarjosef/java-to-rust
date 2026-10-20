public class Caesar {
    static char rotate(char c, int shift) {
        if (c >= 'a' && c <= 'z') {
            return (char) ('a' + (c - 'a' + shift) % 26);
        }
        if (c >= 'A' && c <= 'Z') {
            return (char) ('A' + (c - 'A' + shift) % 26);
        }
        return c;
    }

    static String encrypt(String text, int shift) {
        // Normalize the shift into 0..25 so that negative shifts decrypt.
        int k = (shift % 26 + 26) % 26;
        String out = "";
        for (int i = 0; i < text.length(); i++) {
            out += rotate(text.charAt(i), k);
        }
        return out;
    }

    static String decrypt(String text, int shift) {
        return encrypt(text, -shift);
    }

    static int countLetter(String text, char letter) {
        int count = 0;
        for (int i = 0; i < text.length(); i++) {
            if (Character.toLowerCase(text.charAt(i)) == letter) {
                count++;
            }
        }
        return count;
    }

    static void letterCounts(String text) {
        int letters = 0;
        for (int i = 0; i < text.length(); i++) {
            if (Character.isLetter(text.charAt(i))) {
                letters++;
            }
        }
        System.out.println(letters + " letters");
        for (char c = 'a'; c <= 'z'; c++) {
            int n = countLetter(text, c);
            if (n > 0) {
                String bar = "";
                for (int j = 0; j < n; j++) {
                    bar += '#';
                }
                System.out.println(c + " " + n + " " + bar);
            }
        }
    }

    /** The shift that makes the most common letter an 'e'. */
    static int guessShift(String cipher) {
        char best = 'a';
        int bestCount = -1;
        for (char c = 'a'; c <= 'z'; c++) {
            int n = countLetter(cipher, c);
            if (n > bestCount) {
                best = c;
                bestCount = n;
            }
        }
        return (best - 'e' + 26) % 26;
    }

    public static void main(String[] args) {
        String plain = "The quick brown fox jumps over the lazy dog, then sleeps in the evening breeze.";
        String cipher = encrypt(plain, 3);
        System.out.println(cipher);
        System.out.println(decrypt(cipher, 3));
        System.out.println(encrypt("Zebra-Apple zoo!", 1) + " " + encrypt("abc", -1) + " " + encrypt("xyz", 52) + " " + encrypt("Hi", -27));
        System.out.println(encrypt("Ünïcödé stays: ÀÉ", 13));
        letterCounts(plain);
        int guess = guessShift(cipher);
        System.out.println("guessed shift " + guess + ": " + decrypt(cipher, guess));
        char first = cipher.charAt(0);
        System.out.println(first + " " + (int) first + " " + (char) (first + 1) + " " + (first - 'A'));
        System.out.println(cipher.charAt(cipher.length()));
    }
}
