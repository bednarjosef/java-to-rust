// A static initializer that a `finally` would see fail, whose calls cannot fail for the
// arguments they pass: a varargs constructor called with no elements skips its checks and its
// loop, and a check of a constant argument passes. The JDK's `ImmutableCollections` builds its
// empty collections this way, and `ArraysSupport` checks the scales of arrays.
public class QuietInitializerCalls {
    static class Table {
        static final Table EMPTY = new Table();
        static final int SHIFT = log2(8);
        static final int WORD_SHIFT = log2(Integer.SIZE);

        final int[] slots;
        final int size;

        Table(String... keys) {
            if ((keys.length & 1) != 0) {
                throw new IllegalStateException("odd number of keys: " + keys.length);
            }
            size = keys.length >> 1;
            int len = 2 * keys.length;
            len = (len + 1) & ~1;
            slots = new int[len];
            for (int i = 0; i < keys.length; i += 2) {
                int at = Math.floorMod(keys[i].hashCode(), len);
                if (slots[at] != 0) {
                    throw new IllegalArgumentException("duplicate key: " + keys[i]);
                }
                slots[at] = keys[i + 1].length();
            }
        }

        static int log2(int n) {
            if ((n & (n - 1)) != 0) {
                throw new Error("not a power of two: " + n);
            }
            return Integer.numberOfTrailingZeros(n);
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println(Table.EMPTY.size + " " + Table.EMPTY.slots.length);
            System.out.println(Table.SHIFT + " " + Table.WORD_SHIFT);
            Table t = new Table("a", "one", "b", "three");
            System.out.println(t.size + " " + t.slots.length);
            new Table("x");
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        } finally {
            System.out.println("done");
        }
    }
}
