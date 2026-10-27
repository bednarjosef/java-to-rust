// Locals of one name and different types, one in each arrow case of a switch, as the JDK's
// chunked stores in jdk.internal.foreign.abi.Binding declare them: each case is a scope of its
// own, unlike the cases of a switch with colons, which share one.
public class ArrowCaseLocals {
    static String chunk(int size, long value, int shift) {
        switch (size) {
            case 4 -> {
                int part = (int) (((0xFFFF_FFFFL << shift) & value) >>> shift);
                return "int " + part;
            }
            case 2 -> {
                short part = (short) (((0xFFFFL << shift) & value) >>> shift);
                return "short " + part;
            }
            case 1 -> {
                byte part = (byte) (((0xFFL << shift) & value) >>> shift);
                return "byte " + part;
            }
            default -> {
                return "none";
            }
        }
    }

    // The colon form, whose later case assigns a local that an earlier one declares.
    static int shared(int k) {
        switch (k) {
            case 0:
                int total = 10;
                return total;
            case 1:
                total = 20;
                return total + k;
            default:
                return -1;
        }
    }

    public static void main(String[] args) {
        long value = 0x1234_5678_9ABC_DEF0L;
        for (int size : new int[] {4, 2, 1, 3}) {
            for (int shift : new int[] {0, 8, 16}) {
                System.out.println(size + " " + shift + ": " + chunk(size, value, shift));
            }
        }
        System.out.println(shared(0) + " " + shared(1) + " " + shared(2));
    }
}
