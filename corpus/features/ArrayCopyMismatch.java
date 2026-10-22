public class ArrayCopyMismatch {
    // A copy between two arrays held as Objects. The finally runs before the ArrayStoreException
    // of a type mismatch goes on from the copy.
    static void copy(Object src, int from, Object dest, int to, int n) {
        try {
            System.arraycopy(src, from, dest, to, n);
        } finally {
            System.out.println("copied " + n);
        }
    }

    public static void main(String[] args) {
        Object ints = new int[] {1, 2, 3};
        Object more = new int[5];
        Object words = new String[3];
        copy(ints, 0, more, 1, 3);
        int[] got = (int[]) more;
        for (int i = 0; i < got.length; i++) {
            System.out.print(got[i] + " ");
        }
        System.out.println();
        try {
            copy(ints, 0, words, 0, 2);
        } catch (ArrayStoreException e) {
            System.out.println("caught");
        }
        Object[] refs = {"a", "b"};
        Object[] out = new Object[2];
        copy(refs, 0, out, 0, 2);
        System.out.println(out[0] + " " + out[1]);
        copy(ints, 0, words, 0, 1);
        System.out.println("not reached");
    }
}
