// A store into a null object or array, or out of an array's bounds: Java computes the value,
// with what it prints, before the exception of the store.
public class ArrayStoreValueFirst {
    static class Box { int v; }
    static int effect(String what) {
        System.out.println("evaluated " + what);
        return 1;
    }
    public static void main(String[] args) {
        Box b = args.length > 5 ? new Box() : null;
        try {
            b.v = effect("field value");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        int[] a = args.length > 5 ? new int[3] : null;
        try {
            a[0] = effect("element value");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
        int[] small = new int[1];
        try {
            small[args.length + 4] = effect("out of bounds value");
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            b.v += effect("compound");
        } catch (NullPointerException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
