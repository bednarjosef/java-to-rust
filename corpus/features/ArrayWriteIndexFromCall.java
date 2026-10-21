import java.util.Arrays;

// Writes to an array that has a second name, whose index or value comes from a method or a
// library call given the same array: the call reads the array before the element is written.
public class ArrayWriteIndexFromCall {
    static int last(int[] p) {
        return p.length - 1;
    }

    static int sum(int[] p) {
        int s = 0;
        for (int x : p) {
            s += x;
        }
        return s;
    }

    static int count(double[] p, int k) {
        return Math.floorMod(k, p.length);
    }

    public static void main(String[] args) {
        int[] a = {1, 2, 3};
        int[] alias = a;
        a[last(a)] = 10;
        a[0] = sum(a);
        a[last(alias)] += sum(a);
        a[last(a) - 1]++;
        --a[last(a)];
        int got = a[1] = sum(alias);
        System.out.println(Arrays.toString(alias) + " " + got);

        int[] sorted = {1, 3, 5, 7};
        int[] same = sorted;
        sorted[Arrays.binarySearch(sorted, 5)] = Arrays.hashCode(same) % 100;
        System.out.println(Arrays.toString(same));

        double[][] grid = {{0.5, 1.5}, {2.5}};
        double[] row = grid[0];
        grid[0][count(row, 3)] = count(grid[1], 7) + 0.25;
        row[count(grid[0], 2)] *= count(row, 5) + 2;
        System.out.println(Arrays.deepToString(grid));
    }
}
