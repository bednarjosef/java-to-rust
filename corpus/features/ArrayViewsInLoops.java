// Counting loops over arrays held in locals and parameters, whose indexes are proven in bounds,
// while the body writes the same array, calls methods that write other arrays, the same array
// through another name and statics, leaves early, and replaces a different variable's array.
public class ArrayViewsInLoops {
    static int[] shared = new int[8];
    static int calls;
    static long[] log = new long[4];

    static int touch(int[] other, int k) {
        calls++;
        other[k % other.length] += k;
        log[k % log.length] = (long) k * k;
        shared[k % shared.length] ^= k;
        return k & 3;
    }

    static int scan(int[] values, int[] other) {
        int sum = 0;
        for (int i = 0; i < values.length; i++) {
            values[i] = values[i] * 2 + touch(other, i);
            sum += values[i];
        }
        return sum;
    }

    // The array the loop reads is also the static that the calls write.
    static int aliased() {
        int[] a = shared;
        int sum = 0;
        for (int i = 0; i < a.length; i++) {
            touch(a, i + 1);
            sum += a[i];
        }
        return sum;
    }

    static int firstAbove(int[] values, int limit) {
        for (int i = 0; i < values.length; i++) {
            if (values[i] > limit) {
                return i;
            }
            if (values[i] < 0) {
                break;
            }
        }
        return -1;
    }

    static String[] names = {"ash", "birch", "cedar", "dogwood", "elm"};

    static String joined(String[] parts) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < parts.length; i++) {
            if (parts[i].length() > 4) {
                parts[i] = parts[i].toUpperCase();
            }
            out.append(parts[i]).append(i + 1 < parts.length ? "," : "");
        }
        return out.toString();
    }

    static int rows(int[][] grid) {
        int sum = 0;
        for (int r = 0; r < grid.length; r++) {
            int[] row = grid[r];
            for (int c = 0; c < row.length; c++) {
                row[c] += r;
                sum += row[c];
            }
        }
        return sum;
    }

    static int switched(int[] first, int[] second) {
        int[] other = first;
        int sum = 0;
        for (int i = 0; i < first.length; i++) {
            sum += first[i];
            if (i == 1) {
                other = second;
            }
            sum += other.length;
        }
        return sum;
    }

    static int pastTheEnd(int[] values) {
        int sum = 0;
        for (int i = 0; i <= values.length; i++) {
            sum += values[i];
        }
        return sum;
    }

    public static void main(String[] args) {
        int[] values = {3, 1, 4, 1, 5, 9, 2, 6};
        int[] other = new int[3];
        System.out.println("scan " + scan(values, other) + " " + java.util.Arrays.toString(values));
        System.out.println("other " + java.util.Arrays.toString(other) + " log " + java.util.Arrays.toString(log));
        System.out.println("aliased " + aliased() + " " + java.util.Arrays.toString(shared) + " calls " + calls);
        System.out.println("first " + firstAbove(values, 10) + " " + firstAbove(new int[] {1, -2, 30}, 10)
                + " " + firstAbove(new int[0], 0));
        String[] copy = names.clone();
        System.out.println("joined " + joined(copy) + " " + names[1] + " " + copy[1]);
        int[][] grid = {{1, 2}, {3, 4, 5}, {}};
        System.out.println("rows " + rows(grid) + " " + java.util.Arrays.deepToString(grid));
        System.out.println("switched " + switched(new int[] {1, 2, 3}, new int[5]));
        try {
            System.out.println(pastTheEnd(values));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past the end: " + e.getMessage());
        }
    }
}
