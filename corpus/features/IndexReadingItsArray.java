// Stores into a local array at a position computed from the array itself (`a[n % a.length]`),
// known to be in bounds, of numbers and of objects, and into a row chosen the same way.
public class IndexReadingItsArray {
    static final class Box {
        final int value;

        Box(int value) {
            this.value = value;
        }
    }

    public static void main(String[] args) {
        int[] counts = new int[700];
        Box[] boxes = new Box[4000];
        int[][] grid = new int[30][4];
        for (int round = 0; round < 200; round++) {
            for (int i = 0; i < 300; i++) {
                Box b = new Box(round + i);
                if (i % 15 == 0) {
                    boxes[(round * 20 + i / 15) % boxes.length] = b;
                    counts[(round * 3 + i / 15) % counts.length] = round + i;
                    grid[(round + i / 15) % grid.length][i % 4] = round;
                }
            }
        }
        StringBuilder out = new StringBuilder();
        long sum = 0;
        for (int c : counts) {
            sum = sum * 31 + c;
        }
        for (Box b : boxes) {
            sum = sum * 31 + (b == null ? -1 : b.value);
        }
        out.append(sum).append(' ');
        for (int[] row : grid) {
            for (int v : row) {
                out.append(v).append(' ');
            }
        }
        System.out.println(out.toString().trim());
    }
}
