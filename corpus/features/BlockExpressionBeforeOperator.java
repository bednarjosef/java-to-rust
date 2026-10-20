// An assignment or increment of a static is a block in Rust; where it starts a statement or a
// branch and an operator, a cast or a method call follows, the block must be parenthesised.
public class BlockExpressionBeforeOperator {
    static short sShort = 5;
    static boolean sFlag = true;
    static int sInt = 7;
    static long sLong = -9L;
    static float sFloat = 1.5f;
    static char sChar = 'A';

    static boolean less(boolean c) {
        return c ? ((sShort %= 3) < 1) : (sFloat == sInt);
    }

    static boolean both(boolean c) {
        return c ? true : ((sFlag &= false) && !sFlag);
    }

    static float sum(boolean c) {
        return c ? ((sFloat *= 2) + sInt) : 0.0f;
    }

    static float difference() {
        return (sFloat += 1) - sInt;
    }

    static boolean either() {
        return (sFlag |= false) | sFlag;
    }

    static int remainder() {
        return (sInt++) % 4;
    }

    static boolean same() {
        return (sInt -= 2) == 6;
    }

    static long absolute() {
        return Math.abs(sLong -= 1);
    }

    static double widened() {
        return (sChar++);
    }

    public static void main(String[] args) {
        System.out.println(less(true) + " " + less(false) + " " + sShort);
        System.out.println(both(true) + " " + both(false) + " " + sFlag);
        System.out.println(sum(true) + " " + sum(false) + " " + sFloat);
        System.out.println(difference() + " " + sFloat);
        System.out.println(either() + " " + sFlag);
        System.out.println(remainder() + " " + remainder() + " " + sInt);
        System.out.println(same() + " " + same() + " " + sInt);
        System.out.println(absolute() + " " + sLong);
        System.out.println(widened() + " " + sChar);
    }
}
