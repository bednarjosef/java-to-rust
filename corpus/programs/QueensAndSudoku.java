public class QueensAndSudoku {
    static int nodes;

    /** Counts placements of n queens, one per row, with column and diagonal masks. */
    static int countQueens(int n) {
        boolean[] column = new boolean[n];
        boolean[] slash = new boolean[2 * n];
        boolean[] back = new boolean[2 * n];
        return placeQueen(0, n, column, slash, back);
    }

    static int placeQueen(int row, int n, boolean[] column, boolean[] slash, boolean[] back) {
        if (row == n) {
            return 1;
        }
        int total = 0;
        for (int c = 0; c < n; c++) {
            int s = row + c;
            int b = row - c + n;
            if (column[c] || slash[s] || back[b]) {
                continue;
            }
            column[c] = true;
            slash[s] = true;
            back[b] = true;
            total += placeQueen(row + 1, n, column, slash, back);
            column[c] = false;
            slash[s] = false;
            back[b] = false;
        }
        return total;
    }

    /** The first solution's column for each row, or null when there is none. */
    static int[] firstQueens(int n) {
        int[] cols = new int[n];
        return findQueens(0, n, cols) ? cols : null;
    }

    static boolean findQueens(int row, int n, int[] cols) {
        if (row == n) {
            return true;
        }
        for (int c = 0; c < n; c++) {
            boolean ok = true;
            for (int r = 0; r < row; r++) {
                int d = cols[r] - c;
                if (d == 0 || d == row - r || d == r - row) {
                    ok = false;
                    break;
                }
            }
            if (ok) {
                cols[row] = c;
                if (findQueens(row + 1, n, cols)) {
                    return true;
                }
            }
        }
        return false;
    }

    static int backtracks;

    static boolean canPlace(int[][] grid, int row, int col, int value) {
        for (int i = 0; i < 9; i++) {
            if (grid[row][i] == value || grid[i][col] == value) {
                return false;
            }
        }
        int boxRow = row / 3 * 3;
        int boxCol = col / 3 * 3;
        for (int r = boxRow; r < boxRow + 3; r++) {
            for (int c = boxCol; c < boxCol + 3; c++) {
                if (grid[r][c] == value) {
                    return false;
                }
            }
        }
        return true;
    }

    /** Fills the first empty cell (0) with a legal digit, recursively. */
    static boolean solve(int[][] grid) {
        for (int row = 0; row < 9; row++) {
            for (int col = 0; col < 9; col++) {
                if (grid[row][col] != 0) {
                    continue;
                }
                for (int value = 1; value <= 9; value++) {
                    if (canPlace(grid, row, col, value)) {
                        grid[row][col] = value;
                        if (solve(grid)) {
                            return true;
                        }
                        grid[row][col] = 0;
                        backtracks++;
                    }
                }
                return false;
            }
        }
        return true;
    }

    static int[][] parse(String[] rows) {
        int[][] grid = new int[9][9];
        for (int r = 0; r < 9; r++) {
            for (int c = 0; c < 9; c++) {
                char ch = rows[r].charAt(c);
                grid[r][c] = ch == '.' ? 0 : ch - '0';
            }
        }
        return grid;
    }

    static void printGrid(int[][] grid) {
        for (int r = 0; r < 9; r++) {
            String line = "";
            for (int c = 0; c < 9; c++) {
                line = line + grid[r][c];
                if (c == 2 || c == 5) {
                    line = line + " | ";
                } else if (c < 8) {
                    line = line + " ";
                }
            }
            System.out.println(line);
            if (r == 2 || r == 5) {
                System.out.println("------+-------+------");
            }
        }
    }

    static boolean isValidSolution(int[][] grid) {
        for (int r = 0; r < 9; r++) {
            boolean[] seen = new boolean[10];
            for (int c = 0; c < 9; c++) {
                if (seen[grid[r][c]]) {
                    return false;
                }
                seen[grid[r][c]] = true;
            }
        }
        for (int c = 0; c < 9; c++) {
            boolean[] seen = new boolean[10];
            for (int r = 0; r < 9; r++) {
                if (seen[grid[r][c]]) {
                    return false;
                }
                seen[grid[r][c]] = true;
            }
        }
        return true;
    }

    public static void main(String[] args) {
        for (int n = 4; n <= 9; n++) {
            System.out.println(n + " queens: " + countQueens(n) + " solutions");
        }
        int[] cols = firstQueens(8);
        String board = "";
        for (int r = 0; r < cols.length; r++) {
            board = board + cols[r] + (r + 1 < cols.length ? "," : "");
        }
        System.out.println("first 8-queens columns: " + board);
        System.out.println("no solution for 3 queens: " + (firstQueens(3) == null));

        String[] puzzle = {
            "53..7....",
            "6..195...",
            ".98....6.",
            "8...6...3",
            "4..8.3..1",
            "7...2...6",
            ".6....28.",
            "...419..5",
            "....8..79",
        };
        int[][] grid = parse(puzzle);
        boolean solved = solve(grid);
        System.out.println("solved: " + solved + ", backtracks: " + backtracks);
        printGrid(grid);
        System.out.println("valid: " + isValidSolution(grid));
    }
}
