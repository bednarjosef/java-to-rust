public class EditDistance {
    static int levenshtein(String a, String b) {
        int[][] d = new int[a.length() + 1][b.length() + 1];
        for (int i = 0; i <= a.length(); i++) {
            d[i][0] = i;
        }
        for (int j = 0; j <= b.length(); j++) {
            d[0][j] = j;
        }
        for (int i = 1; i <= a.length(); i++) {
            for (int j = 1; j <= b.length(); j++) {
                int cost = a.charAt(i - 1) == b.charAt(j - 1) ? 0 : 1;
                int del = d[i - 1][j] + 1;
                int ins = d[i][j - 1] + 1;
                int sub = d[i - 1][j - 1] + cost;
                d[i][j] = Math.min(Math.min(del, ins), sub);
            }
        }
        return d[a.length()][b.length()];
    }

    /** Levenshtein distance with two rows instead of a full table. */
    static int levenshteinRows(String a, String b) {
        int[] prev = new int[b.length() + 1];
        int[] cur = new int[b.length() + 1];
        for (int j = 0; j <= b.length(); j++) {
            prev[j] = j;
        }
        for (int i = 1; i <= a.length(); i++) {
            cur[0] = i;
            for (int j = 1; j <= b.length(); j++) {
                int cost = a.charAt(i - 1) == b.charAt(j - 1) ? 0 : 1;
                cur[j] = Math.min(prev[j] + 1, Math.min(cur[j - 1] + 1, prev[j - 1] + cost));
            }
            int[] tmp = prev;
            prev = cur;
            cur = tmp;
        }
        return prev[b.length()];
    }

    /** Damerau variant: an adjacent transposition costs one edit. */
    static int optimalStringAlignment(String a, String b) {
        int n = a.length();
        int m = b.length();
        int[][] d = new int[n + 1][m + 1];
        for (int i = 0; i <= n; i++) {
            d[i][0] = i;
        }
        for (int j = 0; j <= m; j++) {
            d[0][j] = j;
        }
        for (int i = 1; i <= n; i++) {
            for (int j = 1; j <= m; j++) {
                int cost = a.charAt(i - 1) == b.charAt(j - 1) ? 0 : 1;
                d[i][j] = Math.min(d[i - 1][j] + 1, Math.min(d[i][j - 1] + 1, d[i - 1][j - 1] + cost));
                if (i > 1 && j > 1 && a.charAt(i - 1) == b.charAt(j - 2) && a.charAt(i - 2) == b.charAt(j - 1)) {
                    d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
                }
            }
        }
        return d[n][m];
    }

    /** Hamming distance for equal-length strings, or -1 when the lengths differ. */
    static int hamming(String a, String b) {
        if (a.length() != b.length()) {
            return -1;
        }
        int diff = 0;
        for (int i = 0; i < a.length(); i++) {
            if (a.charAt(i) != b.charAt(i)) {
                diff++;
            }
        }
        return diff;
    }

    /** Closest word in a dictionary, ties broken by dictionary order. */
    static String closest(String word, String[] dictionary) {
        String best = null;
        int bestDist = Integer.MAX_VALUE;
        for (String candidate : dictionary) {
            int d = levenshteinRows(word, candidate);
            if (d < bestDist) {
                bestDist = d;
                best = candidate;
            }
        }
        return best;
    }

    public static void main(String[] args) {
        String[][] pairs = {
            {"kitten", "sitting"},
            {"flaw", "lawn"},
            {"intention", "execution"},
            {"", "abc"},
            {"same", "same"},
            {"ca", "abc"},
            {"abcdef", "azced"},
        };
        for (String[] pair : pairs) {
            String a = pair[0];
            String b = pair[1];
            System.out.println(a + " -> " + b + ": " + levenshtein(a, b)
                    + " " + levenshteinRows(a, b)
                    + " osa=" + optimalStringAlignment(a, b));
        }

        String[] dictionary = {"apple", "maple", "ample", "apply", "angle", "ankle", "paddle"};
        String[] misspelled = {"appel", "mapel", "anlge", "padle", "zzz"};
        for (String w : misspelled) {
            System.out.println(w + " is closest to " + closest(w, dictionary));
        }

        System.out.println("hamming " + hamming("karolin", "kathrin") + " " + hamming("abc", "abcd"));

        // A longer pair built from a pattern, to exercise the table.
        StringBuilderLike left = new StringBuilderLike(200);
        StringBuilderLike right = new StringBuilderLike(200);
        for (int i = 0; i < 200; i++) {
            left.add((char) ('a' + (i * 7) % 26));
            right.add((char) ('a' + (i * 11) % 26));
        }
        String x = left.toText();
        String y = right.toText();
        System.out.println("long pair: " + levenshtein(x, y) + " " + levenshteinRows(x, y));
    }

    /** A fixed-capacity character buffer, so the program needs no library class. */
    static class StringBuilderLike {
        private final char[] chars;
        private int size;

        StringBuilderLike(int capacity) {
            chars = new char[capacity];
        }

        void add(char c) {
            chars[size++] = c;
        }

        String toText() {
            String out = "";
            for (int i = 0; i < size; i++) {
                out = out + chars[i];
            }
            return out;
        }
    }
}
