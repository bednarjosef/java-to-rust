// A conditional whose branches are both literals has the type of the conditional, also where a
// cast, a method call or an operator applies to it.
public class ConditionalOfLiterals {
    static boolean sFlag = true;
    static short sShort = 32767;
    static int sInt = 31;

    public static void main(String[] args) {
        boolean b = sFlag;
        System.out.println((short) (b ? 64 : 1234567890123L));
        System.out.println((short) (!b ? 64 : 1234567890123L));
        System.out.println(b ? 3000000000L : 1L);
        System.out.println(Math.abs(b ? -0.1f : 1.5f));
        System.out.println(Math.abs(b ? -2 : 3) + Math.abs(b ? -2.5 : 3.0));
        System.out.println((b ? 2147483647 : 31) + sShort);
        System.out.println((b ? 1000000 : 3) << sInt);
        System.out.println(-(b ? -2147483648 : 5) + sInt);
        System.out.println(~(b ? 4L : 5L));
        System.out.println((int) (b ? 1.5f : 2.5f) + (long) (b ? 1e19 : 2.0));
        System.out.println((b ? 1 : 2) + (b ? 3 : 4) * 1500000000);
        long big = (b ? 4 : 5) * 3000000000L;
        System.out.println(big);
        float f = b ? 0.1f : 0.2f;
        System.out.println((double) f + " " + (double) (b ? 0.1f : 0.2f));
    }
}
