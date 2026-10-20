public class PrimitiveArrays {
    static long[] fibs(int n) {
        long[] f = new long[n];
        for (int i = 0; i < n; i++) f[i] = i < 2 ? i : f[i - 1] + f[i - 2];
        return f;
    }

    static char[] reverse(char[] cs) {
        char[] out = new char[cs.length];
        for (int i = 0; i < cs.length; i++) out[cs.length - 1 - i] = cs[i];
        return out;
    }

    static boolean[] sieve(int n) {
        boolean[] composite = new boolean[n + 1];
        for (int i = 2; i * i <= n; i++) {
            if (!composite[i]) {
                for (int j = i * i; j <= n; j += i) composite[j] = true;
            }
        }
        return composite;
    }

    static double mean(double[] xs) {
        double s = 0;
        for (double x : xs) s += x;
        return xs.length == 0 ? 0 : s / xs.length;
    }

    public static void main(String[] args) {
        long[] f = fibs(12);
        System.out.println(f[11] + " " + f[1] + " " + f.length);
        long[] g = f;
        g[0] = 1000000000000L;
        System.out.println(f[0] * 3);
        char[] word = new char[8];
        for (int i = 0; i < word.length; i++) word[i] = "stressed".charAt(i);
        char[] back = reverse(word);
        for (char ch : back) System.out.print(ch);
        System.out.println(" " + word[0] + back[0]);
        word[0] = 'S';
        for (char ch : word) System.out.print(ch);
        System.out.println(" " + (int) back[7]);
        boolean[] comp = sieve(30);
        int primes = 0;
        for (int i = 2; i <= 30; i++) if (!comp[i]) primes++;
        System.out.println("primes " + primes + " " + comp[4] + " " + comp[7]);
        boolean[] flags = new boolean[3];
        flags[1] = !flags[0];
        System.out.println(flags[0] + " " + flags[1] + " " + flags[2]);
        double[] xs = { 1.5, 2.25, -0.75 };
        System.out.println(mean(xs) + " " + mean(new double[0]));
        xs[2] = xs[0] / 0;
        System.out.println(xs[2] + " " + (xs[2] > 1e300));
        double[] zeros = new double[2];
        System.out.println(zeros[1] + " " + (zeros[0] == -zeros[0]));
        String[] names = new String[3];
        names[1] = "mid";
        System.out.println(names[0] + " " + names[1] + " " + names[2]);
        String[] alias = names;
        alias[0] = names[1] + "!";
        System.out.println(names[0].length() + " " + (names[2] == null));
        long[][] table = new long[2][3];
        table[1][2] = Long.MAX_VALUE;
        table[1][2]++;
        System.out.println(table[1][2] + " " + table[0][0]);
        char c = 'a';
        char[] letters = new char[26];
        for (int i = 0; i < letters.length; i++) letters[i] = (char) (c + i);
        System.out.println(letters[25] + " " + letters[0] + letters[1] + " " + (letters[2] + 1));
    }
}
