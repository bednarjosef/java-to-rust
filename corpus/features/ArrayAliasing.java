public class ArrayAliasing {
    static void bump(int[] v) {
        v[0] += 10;
    }

    static int[] make(int n) {
        int[] made = new int[n];
        made[n - 1] = n;
        return made;
    }

    static int total(int[] v) {
        int sum = 0;
        for (int x : v) {
            sum += x;
        }
        return sum;
    }

    public static void main(String[] args) {
        // Two names for one array: a write through either is seen through both.
        int[] a = new int[3];
        int[] b = a;
        b[0] = 5;
        a[1] = 6;
        System.out.println(a[0] + " " + a[1] + " " + b[0] + " " + b[1]);

        // Passing an array to a method and returning one.
        bump(a);
        int[] c = make(4);
        System.out.println(a[0] + " " + total(c) + " " + c.length);

        // A second local holds the same array.
        int[] alias = c;
        alias[0] = 7;
        System.out.println(c[0] + " " + alias.length);

        // Re-assigning a local makes a new array; the old one stays as it was.
        int[] d = {1, 2, 3};
        int[] old = d;
        d = new int[] {4, 5};
        d[0] += old[2];
        System.out.println(d[0] + " " + d[1] + " " + old[0] + " " + old[2]);

        // A plain array, written while it is read by a for-each over itself.
        int[] p = {1, 2, 3};
        for (int x : p) {
            p[0] += x;
        }
        System.out.println(p[0] + " " + p[1] + " " + p[2]);

        // Indexing with side effects: the index is evaluated before the value.
        int[] q = new int[4];
        int i = 0;
        q[i++] = i;
        q[i++] = i * 10;
        System.out.println(q[0] + " " + q[1] + " " + i);

        // Arrays.fill and Arrays.sort on a local that is also read elsewhere.
        int[] r = new int[4];
        java.util.Arrays.fill(r, 3);
        r[2] = 1;
        java.util.Arrays.sort(r);
        int[] copy = java.util.Arrays.copyOf(r, 6);
        System.out.println(java.util.Arrays.toString(r) + " " + java.util.Arrays.toString(copy));

        // A counting loop over a local array, writing each element.
        int[] s = new int[5];
        for (int k = 0; k < s.length; k++) {
            s[k] = k * 2;
        }
        int acc = 0;
        for (int k = 0; k < s.length; k++) {
            acc += s[k];
        }
        System.out.println(acc + " " + s[4]);

        // The same array held by an array of arrays.
        int[][] grid = {s, new int[2]};
        grid[0][0] = 100;
        System.out.println(s[0] + " " + grid[0].length + " " + grid[1].length);
    }
}
