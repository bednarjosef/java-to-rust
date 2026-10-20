import java.util.Arrays;

public class ArrayClone {
    public static void main(String[] args) {
        int[] a = { 1, 2, 3 };
        int[] b = a.clone();
        b[0] = 99;
        System.out.println(a[0] + " " + b[0] + " " + (a == b));

        String[] words = { "x", "y" };
        String[] copy = words.clone();
        copy[1] = "z";
        System.out.println(words[1] + " " + copy[1] + " " + copy.length);

        double[] d = { 1.5 };
        double[] e = d.clone();
        e[0] += 1;
        System.out.println(d[0] + " " + e[0]);

        char[] cs = { 'h', 'e', 'y' };
        char[] cc = cs.clone();
        cc[0] = 'H';
        System.out.println(cs[0] + "" + cc[0]);

        int[][] grid = { { 1 }, { 2 } };
        int[][] shallow = grid.clone();
        shallow[0][0] = 7;
        System.out.println(grid[0][0] + " " + (shallow[1] == grid[1]));

        System.out.println(Arrays.deepToString(grid));
        String[][] names = { { "a", null }, null };
        System.out.println(Arrays.deepToString(names));
    }
}
