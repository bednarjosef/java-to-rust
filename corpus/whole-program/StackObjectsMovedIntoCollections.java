import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

// Objects made in a loop that only some iterations keep in a JDK collection: each starts on the
// stack and moves to the heap just before the call that keeps it.
public class StackObjectsMovedIntoCollections {
    static final class Pos {
        final int x, y, z;
        Pos(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
        @Override public boolean equals(Object o) {
            return o instanceof Pos p && p.x == x && p.y == y && p.z == z;
        }
        @Override public int hashCode() { return (x * 31 + y) * 31 + z; }
        @Override public String toString() { return "(" + x + ", " + y + ", " + z + ")"; }
    }

    static final class Grid {
        int seed = 42;
        Pos randomPos(int x, int y, int z) {
            seed = seed * 3 + 1013904223;
            int v = seed >> 2;
            return new Pos(x + (v & 15), y + (v >> 16 & 15), z + (v >> 8 & 15));
        }
    }

    public static void main(String[] args) {
        Grid grid = new Grid();
        List<Pos> ticked = new ArrayList<>();
        Map<Pos, Integer> counts = new HashMap<>();
        long sum = 0;
        for (int section = 0; section < 40; section++) {
            for (int k = 0; k < 3; k++) {
                Pos pos = grid.randomPos(section * 16, 64, 0);
                sum += pos.x + pos.y + pos.z;
                if ((pos.x + pos.z) % 7 == 0) {
                    ticked.add(pos);
                }
                if (pos.y % 5 == 0) {
                    counts.put(pos, section);
                }
            }
        }
        System.out.println(sum + " " + ticked.size() + " " + counts.size());
        int same = 0;
        for (Pos p : ticked) {
            Integer n = counts.get(p);
            if (n != null) same += n;
        }
        System.out.println(same + " " + (ticked.isEmpty() ? "none" : ticked.get(0).toString()));
        // The same object is in both: the list's and the map's are one.
        int shared = 0;
        for (Pos p : ticked) {
            for (Pos q : counts.keySet()) {
                if (p == q) shared++;
            }
        }
        System.out.println(shared);
    }
}
