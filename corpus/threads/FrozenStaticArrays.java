// Static final arrays that the program only indexes and measures, read by several threads: a
// table of numbers, a table of objects made by the class's static initialization, which prints,
// a new array of zeros, and a table searched by a loop; beside them a table whose elements are
// written and one used as a value, which stay shared arrays. Indexes out of bounds throw as Java
// throws them.
public class FrozenStaticArrays {
    static final class Point {
        final int x, y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }
    }

    static final class Tables {
        static final Point[] CORNERS = { new Point(0, 0), new Point(1, 0), new Point(1, 1), new Point(0, 1) };
        static final long[] ZEROS = new long[3];
        static final char[] VOWELS = { 'a', 'e', 'i', 'o', 'u' };

        static {
            System.out.println("tables made");
        }

        static boolean vowel(char c) {
            for (int i = 0; i < VOWELS.length; i++) {
                if (VOWELS[i] == c) {
                    return true;
                }
            }
            return false;
        }
    }

    static final int[] SQUARES = { 0, 1, 4, 9, 16, 25, 36, 49 };
    static final int[] HITS = new int[4];
    static final String[] WORDS = { "one", "two", "three" };

    static int sumSquares(int n) {
        int s = 0;
        for (int i = 0; i < n; i++) {
            s += SQUARES[i % SQUARES.length];
        }
        return s;
    }

    static String describe(int k) {
        try {
            Point p = Tables.CORNERS[k];
            return "corner " + k + " at " + p.x + "," + p.y;
        } catch (ArrayIndexOutOfBoundsException e) {
            return "no corner: " + e.getMessage();
        }
    }

    public static void main(String[] args) throws InterruptedException {
        long[] sums = new long[4];
        Thread[] threads = new Thread[4];
        for (int t = 0; t < threads.length; t++) {
            int id = t;
            threads[t] = new Thread(() -> {
                long s = 0;
                for (int round = 0; round < 1000; round++) {
                    s += sumSquares(id + round % 7) + Tables.ZEROS[round % 3] + Tables.CORNERS[round % 4].x;
                }
                sums[id] = s;
                synchronized (HITS) {
                    HITS[id]++;
                }
            });
            threads[t].start();
        }
        for (Thread t : threads) {
            t.join();
        }
        for (int t = 0; t < sums.length; t++) {
            System.out.println("thread " + t + ": " + sums[t] + ", hits " + HITS[t]);
        }
        for (int k = -1; k <= 4; k++) {
            System.out.println(describe(k));
        }
        System.out.println(Tables.vowel('e') + " " + Tables.vowel('x') + " " + Tables.ZEROS.length);
        try {
            System.out.println(SQUARES[8]);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past the squares: " + e.getMessage());
        }
        for (String w : WORDS) {
            System.out.print(w + " ");
        }
        System.out.println(WORDS.length);
    }
}
