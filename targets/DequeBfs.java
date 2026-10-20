// Features: ArrayDeque as a stack and as a queue (push, pop, offer, poll, peekFirst), breadth-first
// and depth-first search on a char grid, int[] pairs, 2D arrays, labelled loops, Deque iteration.
import java.util.ArrayDeque;
import java.util.Deque;

public class DequeBfs {
    static final String[] MAZE = {
        "S..#......",
        ".#.#.####.",
        ".#...#....",
        ".####.#.#.",
        "......#.#E",
        "#.######..",
        "..........",
    };

    static final int[][] STEPS = {{0, 1}, {1, 0}, {0, -1}, {-1, 0}};

    static int rows() {
        return MAZE.length;
    }

    static int cols() {
        return MAZE[0].length();
    }

    static int[] find(char target) {
        for (int r = 0; r < rows(); r++) {
            for (int c = 0; c < cols(); c++) {
                if (MAZE[r].charAt(c) == target) {
                    return new int[] {r, c};
                }
            }
        }
        throw new IllegalStateException("no " + target + " in maze");
    }

    static boolean open(int r, int c) {
        return r >= 0 && c >= 0 && r < rows() && c < cols() && MAZE[r].charAt(c) != '#';
    }

    static int shortestPath() {
        int[] start = find('S');
        int[] goal = find('E');
        int[][] dist = new int[rows()][cols()];
        for (int[] row : dist) {
            java.util.Arrays.fill(row, -1);
        }
        Deque<int[]> queue = new ArrayDeque<>();
        queue.offer(start);
        dist[start[0]][start[1]] = 0;
        while (!queue.isEmpty()) {
            int[] cur = queue.poll();
            if (cur[0] == goal[0] && cur[1] == goal[1]) {
                return dist[cur[0]][cur[1]];
            }
            for (int[] step : STEPS) {
                int nr = cur[0] + step[0];
                int nc = cur[1] + step[1];
                if (open(nr, nc) && dist[nr][nc] < 0) {
                    dist[nr][nc] = dist[cur[0]][cur[1]] + 1;
                    queue.offer(new int[] {nr, nc});
                }
            }
        }
        return -1;
    }

    static int countReachable() {
        int[] start = find('S');
        boolean[][] seen = new boolean[rows()][cols()];
        Deque<int[]> stack = new ArrayDeque<>();
        stack.push(start);
        seen[start[0]][start[1]] = true;
        int count = 0;
        while (!stack.isEmpty()) {
            int[] cur = stack.pop();
            count++;
            for (int[] step : STEPS) {
                int nr = cur[0] + step[0];
                int nc = cur[1] + step[1];
                if (open(nr, nc) && !seen[nr][nc]) {
                    seen[nr][nc] = true;
                    stack.push(new int[] {nr, nc});
                }
            }
        }
        return count;
    }

    static String reverseWords(String text) {
        Deque<String> words = new ArrayDeque<>();
        for (String w : text.split(" ")) {
            words.addFirst(w);
        }
        StringBuilder sb = new StringBuilder();
        for (String w : words) {
            if (sb.length() > 0) {
                sb.append(' ');
            }
            sb.append(w);
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println("maze " + rows() + "x" + cols());
        System.out.println("shortest path: " + shortestPath());
        System.out.println("reachable cells: " + countReachable());
        System.out.println(reverseWords("deques are double ended"));

        Deque<Integer> stack = new ArrayDeque<>();
        for (int i = 1; i <= 5; i++) {
            stack.push(i * i);
        }
        System.out.println("stack top: " + stack.peek() + " size " + stack.size());
        StringBuilder popped = new StringBuilder();
        while (!stack.isEmpty()) {
            popped.append(stack.pop()).append(' ');
        }
        System.out.println("popped: " + popped.toString().trim());
        System.out.println("pop on empty: " + stack.poll());
        try {
            stack.pop();
        } catch (java.util.NoSuchElementException e) {
            System.out.println("pop on empty throws");
        }

        Deque<String> line = new ArrayDeque<>();
        line.offerLast("ann");
        line.offerLast("ben");
        line.offerFirst("cy");
        line.offerLast("dot");
        System.out.println("line: " + line + " peekLast " + line.peekLast());
        int served = 0;
        while (line.size() > 1) {
            String front = line.pollFirst();
            if (front.length() == 3 && served % 2 == 0) {
                line.offerLast(front + "*");
            }
            served++;
        }
        System.out.println("last one: " + line.peekFirst() + " after " + served);
    }
}
