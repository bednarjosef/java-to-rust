// Arrays that another thread reaches (a worker's field, a static, an Object that threads share)
// and arrays that stay on the main thread, in one program with threads.
public class LocalArraysWithThreads {
    static Object latest;

    static class Summer extends Thread {
        private final int[][] grid;
        long total;

        Summer(int[][] grid) {
            this.grid = grid;
        }

        @Override
        public void run() {
            for (int[] row : grid) {
                for (int v : row) {
                    total += v;
                }
            }
            latest = new double[] {total / 2.0};
        }
    }

    public static void main(String[] args) throws InterruptedException {
        int[][] grid = new int[3][4];
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 4; j++) {
                grid[i][j] = i * 4 + j;
            }
        }
        Summer s = new Summer(grid);
        s.start();
        s.join();
        System.out.println("total " + s.total + ", half " + ((double[]) latest)[0]);

        String[] words = "the quick brown fox".split(" ");
        char[] first = new char[words.length];
        for (int i = 0; i < words.length; i++) {
            first[i] = words[i].charAt(0);
        }
        char[] same = first;
        boolean[] seen = new boolean[26];
        for (char c : first) {
            seen[c - 'a'] = true;
        }
        int count = 0;
        for (boolean b : seen) {
            if (b) {
                count++;
            }
        }
        System.out.println(new String(first) + " " + (same == first) + " " + count);
        byte[] bytes = "hi".getBytes();
        System.out.println(bytes.length + " " + bytes[0]);
    }
}
