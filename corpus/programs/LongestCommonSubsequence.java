public class LongestCommonSubsequence {
    static int[][] table(String a, String b) {
        int[][] len = new int[a.length() + 1][b.length() + 1];
        for (int i = 1; i <= a.length(); i++) {
            for (int j = 1; j <= b.length(); j++) {
                if (a.charAt(i - 1) == b.charAt(j - 1)) {
                    len[i][j] = len[i - 1][j - 1] + 1;
                } else if (len[i - 1][j] >= len[i][j - 1]) {
                    len[i][j] = len[i - 1][j];
                } else {
                    len[i][j] = len[i][j - 1];
                }
            }
        }
        return len;
    }

    /** Rebuilds one longest common subsequence by walking the table backwards. */
    static String reconstruct(String a, String b) {
        int[][] len = table(a, b);
        char[] out = new char[len[a.length()][b.length()]];
        int k = out.length;
        int i = a.length();
        int j = b.length();
        while (i > 0 && j > 0) {
            if (a.charAt(i - 1) == b.charAt(j - 1)) {
                out[--k] = a.charAt(i - 1);
                i--;
                j--;
            } else if (len[i - 1][j] >= len[i][j - 1]) {
                i--;
            } else {
                j--;
            }
        }
        return toText(out);
    }

    /** Length of the longest common substring (contiguous), computed with a rolling row. */
    static int longestCommonSubstring(String a, String b) {
        int[] row = new int[b.length() + 1];
        int best = 0;
        for (int i = 1; i <= a.length(); i++) {
            for (int j = b.length(); j >= 1; j--) {
                if (a.charAt(i - 1) == b.charAt(j - 1)) {
                    row[j] = row[j - 1] + 1;
                    best = Math.max(best, row[j]);
                } else {
                    row[j] = 0;
                }
            }
        }
        return best;
    }

    /** Longest increasing subsequence in O(n^2). */
    static int longestIncreasing(int[] seq) {
        int[] len = new int[seq.length];
        int best = 0;
        for (int i = 0; i < seq.length; i++) {
            len[i] = 1;
            for (int j = 0; j < i; j++) {
                if (seq[j] < seq[i] && len[j] + 1 > len[i]) {
                    len[i] = len[j] + 1;
                }
            }
            best = Math.max(best, len[i]);
        }
        return best;
    }

    static char[] randomText(long seed, int length, int alphabet) {
        char[] text = new char[length];
        long state = seed;
        for (int i = 0; i < length; i++) {
            state = (state * 6364136223846793005L + 1442695040888963407L);
            int r = (int) ((state >>> 33) % alphabet);
            text[i] = (char) ('a' + r);
        }
        return text;
    }

    static String toText(char[] chars) {
        String text = "";
        for (char c : chars) {
            text = text + c;
        }
        return text;
    }

    public static void main(String[] args) {
        String[][] pairs = {
            {"ABCBDAB", "BDCABA"},
            {"AGGTAB", "GXTXAYB"},
            {"", "xyz"},
            {"banana", "atana"},
            {"programming", "gaming"},
        };
        for (String[] p : pairs) {
            System.out.println(p[0] + " / " + p[1] + ": " + table(p[0], p[1])[p[0].length()][p[1].length()]
                    + " \"" + reconstruct(p[0], p[1]) + "\""
                    + " substring " + longestCommonSubstring(p[0], p[1]));
        }

        String sa = toText(randomText(7, 300, 4));
        String sb = toText(randomText(99, 300, 4));
        int lcs = table(sa, sb)[sa.length()][sb.length()];
        System.out.println("random 300x300 lcs = " + lcs);
        System.out.println("lcs ratio = " + (100 * lcs / 300) + "%");

        int[] seq = new int[40];
        long state = 2024;
        String listing = "";
        for (int i = 0; i < seq.length; i++) {
            state = (state * 25214903917L + 11) & ((1L << 48) - 1);
            seq[i] = (int) (state >>> 17) % 100;
            listing = listing + seq[i] + " ";
        }
        System.out.println("sequence: " + listing);
        System.out.println("lis = " + longestIncreasing(seq));
    }
}
