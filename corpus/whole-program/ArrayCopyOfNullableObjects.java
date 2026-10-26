// System.arraycopy between Objects that may be null, which the copy reads through a reference
// to the checked value while the other side is a plain value: the copy across array classes
// takes both as the program's Object.
public class ArrayCopyOfNullableObjects {
    Object items = new String[] {"a", "b", "c"};

    static int copied(Object from, Object to, int n) {
        try {
            System.arraycopy(from, 0, to, 0, n);
            return n;
        } catch (ArrayStoreException e) {
            return -1;
        }
    }

    void grow(Object added) {
        Object[] next = new Object[4];
        System.arraycopy(items, 0, next, 0, 3);
        System.arraycopy(added, 0, next, 3, 1);
        items = next;
    }

    public static void main(String[] args) {
        ArrayCopyOfNullableObjects m = new ArrayCopyOfNullableObjects();
        if (args.length > 5) {
            m.items = null;
        }
        Object added = args.length > 0 ? new int[] {1} : new CharSequence[] {"d"};
        m.grow(added);
        System.out.println(java.util.Arrays.toString((Object[]) m.items));
        Object numbers = new Integer[] {1, 2};
        System.out.println(copied(numbers, new String[2], 2) + " " + copied(m.items, new Object[4], 4));
    }
}
