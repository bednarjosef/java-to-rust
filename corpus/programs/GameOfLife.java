public class GameOfLife {
    static final int ROWS = 12;
    static final int COLS = 20;

    static boolean[][] blank() {
        return new boolean[ROWS][COLS];
    }

    static int neighbours(boolean[][] g, int r, int c) {
        int count = 0;
        for (int dr = -1; dr <= 1; dr++) {
            for (int dc = -1; dc <= 1; dc++) {
                if (dr == 0 && dc == 0) {
                    continue;
                }
                int rr = (r + dr + ROWS) % ROWS;
                int cc = (c + dc + COLS) % COLS;
                if (g[rr][cc]) {
                    count++;
                }
            }
        }
        return count;
    }

    static boolean[][] step(boolean[][] g) {
        boolean[][] next = blank();
        for (int r = 0; r < ROWS; r++) {
            for (int c = 0; c < COLS; c++) {
                int n = neighbours(g, r, c);
                if (g[r][c]) {
                    next[r][c] = n == 2 || n == 3;
                } else {
                    next[r][c] = n == 3;
                }
            }
        }
        return next;
    }

    static int population(boolean[][] g) {
        int count = 0;
        for (boolean[] row : g) {
            for (boolean alive : row) {
                if (alive) {
                    count++;
                }
            }
        }
        return count;
    }

    static void place(boolean[][] g, String[] pattern, int top, int left) {
        for (int r = 0; r < pattern.length; r++) {
            for (int c = 0; c < pattern[r].length(); c++) {
                if (pattern[r].charAt(c) == '#') {
                    g[top + r][left + c] = true;
                }
            }
        }
    }

    static void show(boolean[][] g) {
        for (int r = 0; r < ROWS; r++) {
            String line = "";
            for (int c = 0; c < COLS; c++) {
                line = line + (g[r][c] ? '#' : '.');
            }
            System.out.println(line);
        }
    }

    static boolean same(boolean[][] a, boolean[][] b) {
        for (int r = 0; r < ROWS; r++) {
            for (int c = 0; c < COLS; c++) {
                if (a[r][c] != b[r][c]) {
                    return false;
                }
            }
        }
        return true;
    }

    public static void main(String[] args) {
        String[] glider = {
            ".#.",
            "..#",
            "###",
        };
        String[] blinker = {
            "###",
        };
        String[] rPentomino = {
            ".##",
            "##.",
            ".#.",
        };

        boolean[][] world = blank();
        place(world, glider, 1, 1);
        place(world, blinker, 8, 12);
        System.out.println("generation 0, population " + population(world));
        show(world);

        for (int gen = 1; gen <= 8; gen++) {
            world = step(world);
        }
        System.out.println("generation 8, population " + population(world));
        show(world);

        boolean[][] pent = blank();
        place(pent, rPentomino, 5, 8);
        int maxPop = 0;
        int genOfMax = 0;
        boolean[][] previous = pent;
        int stableAt = -1;
        for (int gen = 1; gen <= 200; gen++) {
            boolean[][] next = step(previous);
            int p = population(next);
            if (p > maxPop) {
                maxPop = p;
                genOfMax = gen;
            }
            if (stableAt < 0 && same(next, previous)) {
                stableAt = gen;
            }
            previous = next;
        }
        System.out.println("r-pentomino: max population " + maxPop + " at generation " + genOfMax);
        System.out.println("final population " + population(previous) + ", still changing at 200: " + (stableAt < 0));
    }
}
