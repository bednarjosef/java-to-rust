// A generic linked queue drives a breadth-first search over a grid maze; a generic pair holds
// each step, and a generic counter tallies what the search saw.
public class LinkedQueue {
    static class Node<T> {
        final T value;
        Node<T> next;
        Node(T value) { this.value = value; }
    }

    static class Queue<T> {
        private Node<T> head;
        private Node<T> tail;
        private int size;

        void add(T value) {
            Node<T> node = new Node<>(value);
            if (tail == null) {
                head = node;
            } else {
                tail.next = node;
            }
            tail = node;
            size++;
        }

        T poll() {
            if (head == null) return null;
            T value = head.value;
            head = head.next;
            if (head == null) tail = null;
            size--;
            return value;
        }

        boolean isEmpty() { return size == 0; }
        int size() { return size; }
    }

    static class Pair<A, B> {
        final A left;
        final B right;
        Pair(A left, B right) { this.left = left; this.right = right; }
        A left() { return left; }
        B right() { return right; }
    }

    static class Point {
        final int row;
        final int col;
        Point(int row, int col) { this.row = row; this.col = col; }
    }

    static class Tally<K> {
        private K best;
        private int bestCount;
        private int total;

        void offer(K key, int count) {
            total += count;
            if (best == null || count > bestCount) {
                best = key;
                bestCount = count;
            }
        }

        K best() { return best; }
        int total() { return total; }
    }

    static final String[] MAZE = {
        "S.#.......",
        ".##.####.#",
        "....#..#..",
        "#.#.#.##.#",
        "..#...#...",
        ".####.#.#.",
        "......#.#E",
    };

    static <T> T firstNonNull(T a, T b) { return a != null ? a : b; }

    public static void main(String[] args) {
        int rows = MAZE.length;
        int cols = MAZE[0].length();
        int[][] dist = new int[rows][cols];
        for (int[] row : dist) java.util.Arrays.fill(row, -1);
        Queue<Pair<Point, Integer>> queue = new Queue<>();
        queue.add(new Pair<>(new Point(0, 0), 0));
        dist[0][0] = 0;
        int[] dr = {1, -1, 0, 0};
        int[] dc = {0, 0, 1, -1};
        int visited = 0;
        int widest = 0;
        Point end = null;
        while (!queue.isEmpty()) {
            widest = Math.max(widest, queue.size());
            Pair<Point, Integer> step = queue.poll();
            Point p = step.left();
            visited++;
            if (MAZE[p.row].charAt(p.col) == 'E') {
                end = p;
                break;
            }
            for (int d = 0; d < 4; d++) {
                int r = p.row + dr[d];
                int c = p.col + dc[d];
                if (r < 0 || r >= rows || c < 0 || c >= cols) continue;
                if (MAZE[r].charAt(c) == '#' || dist[r][c] >= 0) continue;
                dist[r][c] = step.right() + 1;
                queue.add(new Pair<>(new Point(r, c), step.right() + 1));
            }
        }
        System.out.println("visited " + visited + ", widest frontier " + widest);
        if (end != null) {
            System.out.println("exit at " + end.row + "," + end.col + " after " + dist[end.row][end.col] + " steps");
        }
        for (int r = 0; r < rows; r++) {
            StringBuilder line = new StringBuilder();
            for (int c = 0; c < cols; c++) {
                if (MAZE[r].charAt(c) == '#') {
                    line.append('#');
                } else if (dist[r][c] < 0) {
                    line.append(' ');
                } else {
                    line.append((char) ('0' + dist[r][c] % 10));
                }
            }
            System.out.println(line);
        }

        Tally<String> words = new Tally<>();
        String[] names = {"north", "south", "east", "west"};
        for (int d = 0; d < 4; d++) {
            int open = 0;
            for (int r = 0; r < rows; r++) {
                for (int c = 0; c < cols; c++) {
                    int nr = r + dr[d];
                    int nc = c + dc[d];
                    if (MAZE[r].charAt(c) != '#' && nr >= 0 && nr < rows && nc >= 0 && nc < cols
                            && MAZE[nr].charAt(nc) != '#') {
                        open++;
                    }
                }
            }
            words.offer(names[d], open);
        }
        System.out.println("most open direction " + words.best() + " of " + words.total() + " moves");

        Tally<Integer> lengths = new Tally<>();
        for (String row : MAZE) {
            int run = 0;
            int longest = 0;
            for (int c = 0; c < row.length(); c++) {
                run = row.charAt(c) == '#' ? 0 : run + 1;
                longest = Math.max(longest, run);
            }
            lengths.offer(longest, longest);
        }
        System.out.println("longest corridor " + lengths.best() + ", corridor total " + lengths.total());

        Queue<String> labels = new Queue<>();
        labels.add("first");
        labels.add(null);
        labels.add("third");
        String a = labels.poll();
        String b = labels.poll();
        String c = labels.poll();
        System.out.println(firstNonNull(b, a) + " " + firstNonNull(b, c) + " " + labels.poll() + " " + labels.size());
    }
}
