// A do-while whose body reads the length of an array it has just read from a field that may be
// null, which leaves it known not to be null for the rest of the body, and whose condition reads
// the array again, as netty's AdaptivePoolingAllocator.MagazineGroup.allocate does.
public class DoWhileConditionAfterNarrowing {
    volatile int[][] slots = {{1, 2}, {3}};
    int expansions;

    boolean expand(int length) {
        if (length >= 4) {
            return false;
        }
        int[][] grown = new int[length * 2][];
        for (int i = 0; i < grown.length; i++) {
            grown[i] = new int[] {i * 10};
        }
        slots = grown;
        return true;
    }

    int find(int wanted) {
        int[][] slots;
        int seen = 0;
        do {
            slots = this.slots;
            int mask = slots.length - 1;
            for (int i = 0, m = slots.length << 1; i < m; i++) {
                int[] slot = slots[i & mask];
                seen += slot[0];
                if (slot[0] == wanted) {
                    return seen;
                }
            }
            expansions++;
        } while (expansions <= 3 && expand(slots.length));
        return -seen;
    }

    void clear() {
        slots = null;
    }

    public static void main(String[] args) {
        System.out.println(new DoWhileConditionAfterNarrowing().find(20));
        System.out.println(new DoWhileConditionAfterNarrowing().find(99));
        DoWhileConditionAfterNarrowing cleared = new DoWhileConditionAfterNarrowing();
        cleared.clear();
        try {
            System.out.println(cleared.find(1));
        } catch (NullPointerException e) {
            System.out.println("cleared: " + e.getMessage());
        }
    }
}
