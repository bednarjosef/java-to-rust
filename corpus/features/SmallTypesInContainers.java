class Letter {
    char c;
    byte weight;
    float score;
    static short made;

    Letter(char c, int weight) {
        this.c = c;
        this.weight = (byte) weight;
        this.score = weight * 0.5f;
        made++;
    }

    char next() {
        c++;
        return c;
    }

    public String toString() {
        return c + ":" + weight + ":" + score;
    }
}

public class SmallTypesInContainers {
    static int countVowels(char[] cs) {
        int n = 0;
        for (char c : cs) {
            if (c == 'a' || c == 'e' || c == 'i' || c == 'o' || c == 'u') {
                n++;
            }
        }
        return n;
    }

    public static void main(String[] args) {
        char[] cs = new char[5];
        String word = "queue";
        for (int i = 0; i < cs.length; i++) {
            cs[i] = word.charAt(i);
        }
        System.out.println(countVowels(cs) + " " + cs[0] + cs[4]);
        byte[] bs = {1, 2, 127};
        bs[2]++;
        short[] ss = new short[2];
        ss[0] = (short) 40000;
        float[] fs = {1.5f, 2.25f};
        fs[1] *= 3;
        System.out.println(bs[2] + " " + ss[0] + " " + ss[1] + " " + fs[1] + " " + java.util.Arrays.toString(fs));
        System.out.println(java.util.Arrays.toString(bs) + java.util.Arrays.toString(cs));
        Letter l = new Letter('y', 300);
        l.next();
        System.out.println(l + " " + l.next() + " " + Letter.made);
        Letter m = l;
        m.score += 1;
        System.out.println(l.score + " " + new Letter('a', 3));
        char[][] grid = new char[2][3];
        grid[1][2] = 'z';
        System.out.println((int) grid[0][0] + " " + grid[1][2]);
        float zero = 0;
        float[] g = {3, -0.0f, zero / zero, 0.0f, -1, -zero / zero};
        java.util.Arrays.sort(g);
        double[] d = {zero / zero, -0.0, 0.0, -2};
        java.util.Arrays.sort(d);
        char[] letters = word.length() > 0 ? cs : new char[0];
        java.util.Arrays.sort(letters);
        System.out.println(java.util.Arrays.toString(g) + java.util.Arrays.toString(d) + java.util.Arrays.toString(letters));
    }
}
