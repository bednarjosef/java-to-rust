// `-` and `~` of a literal give a literal of the operand's type, whatever the context casts it to
// or calls on it.
public class UnaryOperatorOnLiteral {
    static int sInt = 255;
    static byte sByte = (byte) -3;
    static char sChar = 'Z';
    static long sLong = 1234567890123L;

    public static void main(String[] args) {
        sInt >>>= ~9223372036854775807L;
        System.out.println(sInt);
        long sum = (~9223372036854775807L) + sLong;
        System.out.println(sum);
        int wrapped = (-2147483647 - 1) + sByte;
        System.out.println(wrapped);
        System.out.println((-32) + sByte);
        System.out.println((-7) * sChar);
        System.out.println((-0) - sByte);
        System.out.println((byte) ((~9223372036854775807L) + (-300)));
        System.out.println((char) (-(65)) + 0);
        System.out.println((short) (-(1.5e10)));
        System.out.println(-(-(2147483647)) + " " + -(-2147483648) + " " + ~(~5L));
        System.out.println((double) (-(3.5f)) + " " + (int) (-(0.0)) + " " + (1 / -(0.0)));
    }
}
