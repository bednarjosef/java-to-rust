// Arrays handed from one variable to another: a move where the old name is never read again,
// a second name for the same array where it is.
public class ArrayMovesAndAliases {
    static int[] numbers(int n) {
        int[] a = new int[n];
        for (int i = 0; i < n; i++) {
            a[i] = i * i;
        }
        return a;
    }

    static int[] keep;

    static void remember(int[] a) {
        keep = a;
    }

    static int first(int[] a) {
        return a[0];
    }

    static int[] same(int[] a) {
        return a;
    }

    static int[][] grid(int n) {
        int[][] g = new int[n][];
        for (int i = 0; i < n; i++) {
            g[i] = new int[i + 1];
        }
        return g;
    }

    public static void main(String[] args) {
        // Two names that both stay in use see each other's writes.
        int[] a = numbers(4);
        int[] b = a;
        b[0] = 100;
        System.out.println(a[0] + " " + b[0]);

        // Rotating three arrays: every old value is overwritten before it is read again.
        int[] x = numbers(3);
        int[] y = numbers(5);
        for (int round = 0; round < 4; round++) {
            int[] z = new int[x.length + y.length];
            for (int i = 0; i < x.length; i++) {
                z[i] = x[i];
            }
            for (int i = 0; i < y.length; i++) {
                z[x.length + i] = y[i] + round;
            }
            x = y;
            y = z;
        }
        System.out.println(x.length + " " + y.length + " " + y[y.length - 1] + " " + x[0]);

        // A swap through a temporary.
        int[] p = {1, 2};
        int[] q = {3, 4, 5};
        for (int i = 0; i < 3; i++) {
            int[] t = p;
            p = q;
            q = t;
            p[0] += 10;
        }
        System.out.println(p.length + " " + p[0] + " " + q.length + " " + q[0]);

        // Moved only on one branch: the other branch still reads the old array.
        int[] r = numbers(3);
        int[] s;
        if (r.length > 2) {
            s = r;
            s[1] = 42;
        } else {
            s = new int[1];
        }
        System.out.println(s[1] + " " + s.length);

        // Kept in a static field: the array is shared from then on.
        int[] kept = numbers(2);
        remember(kept);
        keep[1] = 77;
        System.out.println(keep[1]);
        int[] kept2 = numbers(2);
        remember(kept2);
        kept2[0] = 5;
        System.out.println(keep[0] + " " + first(kept2));

        // A method that hands back its parameter returns the caller's array.
        int[] orig = numbers(3);
        int[] back = same(orig);
        back[2] = -1;
        System.out.println(orig[2]);

        // A row kept in a variable is the same row as in the grid.
        int[][] g = grid(3);
        int[] row = g[2];
        row[1] = 9;
        System.out.println(g[2][1] + " " + g[0].length + " " + g[2].length);
        int[][] h = new int[2][2];
        h[0] = h[1];
        h[0][0] = 8;
        System.out.println(h[1][0]);

        // An array moved into a call whose parameter keeps it.
        int[] gone = numbers(3);
        remember(gone);
        System.out.println(keep[2]);
        // Comparing arrays by identity.
        int[] c1 = numbers(1);
        int[] c2 = c1;
        System.out.println((c1 == c2) + " " + (c1 == numbers(1)));
    }
}
