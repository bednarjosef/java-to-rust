public class MazeBfs {
    static final String[] MAZE = {
        "##########",
        "#S.....#.#",
        "#.##.#.#.#",
        "#.#..#...#",
        "#.#.####.#",
        "#...#..#.#",
        "###.#.##.#",
        "#.....#..#",
        "#.###...E#",
        "##########",
    };

    static int rows() {
        return MAZE.length;
    }

    static int cols() {
        return MAZE[0].length();
    }

    static char at(int r, int c) {
        return MAZE[r].charAt(c);
    }

    /** Breadth-first search with an array as the queue; returns distances, -1 when unreachable. */
    static int[][] distances(int sr, int sc) {
        int r = rows();
        int c = cols();
        int[][] dist = new int[r][c];
        for (int i = 0; i < r; i++) {
            for (int j = 0; j < c; j++) {
                dist[i][j] = -1;
            }
        }
        int[] queue = new int[r * c];
        int head = 0;
        int tail = 0;
        queue[tail++] = sr * c + sc;
        dist[sr][sc] = 0;
        int[] dr = {-1, 1, 0, 0};
        int[] dc = {0, 0, -1, 1};
        while (head < tail) {
            int cur = queue[head++];
            int cr = cur / c;
            int cc = cur % c;
            for (int k = 0; k < 4; k++) {
                int nr = cr + dr[k];
                int nc = cc + dc[k];
                if (nr < 0 || nc < 0 || nr >= r || nc >= c) {
                    continue;
                }
                if (at(nr, nc) == '#' || dist[nr][nc] != -1) {
                    continue;
                }
                dist[nr][nc] = dist[cr][cc] + 1;
                queue[tail++] = nr * c + nc;
            }
        }
        return dist;
    }

    static int[] find(char wanted) {
        for (int r = 0; r < rows(); r++) {
            for (int c = 0; c < cols(); c++) {
                if (at(r, c) == wanted) {
                    return new int[] {r, c};
                }
            }
        }
        return null;
    }

    /** Marks one shortest path with '*' on a copy of the maze and returns the copy. */
    static String[] shortestPathMap(int[][] dist, int tr, int tc) {
        char[][] canvas = new char[rows()][cols()];
        for (int r = 0; r < rows(); r++) {
            for (int c = 0; c < cols(); c++) {
                canvas[r][c] = at(r, c);
            }
        }
        int r = tr;
        int c = tc;
        while (dist[r][c] > 0) {
            canvas[r][c] = '*';
            if (r > 0 && dist[r - 1][c] == dist[r][c] - 1) {
                r--;
            } else if (r + 1 < rows() && dist[r + 1][c] == dist[r][c] - 1) {
                r++;
            } else if (c > 0 && dist[r][c - 1] == dist[r][c] - 1) {
                c--;
            } else {
                c++;
            }
        }
        String[] out = new String[canvas.length];
        for (int i = 0; i < canvas.length; i++) {
            String line = "";
            for (int col = 0; col < cols(); col++) {
                line = line + canvas[i][col];
            }
            out[i] = line;
        }
        return out;
    }

    public static void main(String[] args) {
        int[] start = find('S');
        int[] end = find('E');
        int[][] dist = distances(start[0], start[1]);
        System.out.println("start " + start[0] + "," + start[1] + " end " + end[0] + "," + end[1]);
        System.out.println("shortest path: " + dist[end[0]][end[1]]);

        int reachable = 0;
        int farthest = 0;
        for (int r = 0; r < rows(); r++) {
            for (int c = 0; c < cols(); c++) {
                if (dist[r][c] >= 0) {
                    reachable++;
                    farthest = Math.max(farthest, dist[r][c]);
                }
            }
        }
        System.out.println("reachable cells " + reachable + ", farthest " + farthest);

        for (String line : shortestPathMap(dist, end[0], end[1])) {
            System.out.println(line);
        }

        // A larger generated grid: walls where a hash of the cell is odd.
        int n = 300;
        int[][] big = new int[n][n];
        boolean[][] wall = new boolean[n][n];
        for (int r = 0; r < n; r++) {
            for (int c = 0; c < n; c++) {
                int h = (r * 73856093) ^ (c * 19349663);
                wall[r][c] = (h & 7) == 0 && !(r == 0 && c == 0);
            }
        }
        int[] queue = new int[n * n];
        int head = 0;
        int tail = 0;
        queue[tail++] = 0;
        big[0][0] = 1;
        int[] dr = {1, 0, -1, 0};
        int[] dc = {0, 1, 0, -1};
        while (head < tail) {
            int cur = queue[head++];
            for (int k = 0; k < 4; k++) {
                int nr = cur / n + dr[k];
                int nc = cur % n + dc[k];
                if (nr >= 0 && nc >= 0 && nr < n && nc < n && !wall[nr][nc] && big[nr][nc] == 0) {
                    big[nr][nc] = big[cur / n][cur % n] + 1;
                    queue[tail++] = nr * n + nc;
                }
            }
        }
        System.out.println("grid reached " + tail + " cells; corner distance " + big[n - 1][n - 1]);
    }
}
